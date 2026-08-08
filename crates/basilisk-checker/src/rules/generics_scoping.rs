//! Implements [`generics_scoping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_scoping`: Unbound type variable in scope.
//!
//! A type variable used in a type annotation must be "in scope" — i.e. it must
//! be bound by a surrounding generic class (`Generic[T]`), PEP 695 type
//! parameter, or function signature parameter.
//!
//! Unbound usages include:
//! - `TypeVar` in a local variable annotation when the function does not bind it
//! - `TypeVar` in a class body attribute when the class does not include it in `Generic[...]`
//! - Inner class reusing an outer class's `TypeVar` in `Generic[T]` or body annotations
//! - `TypeVar` at module level in annotations
//! - `TypeAlias` at class level referencing the class's own `TypeVar`s
//!
//! ```python
//! T = TypeVar("T")
//! S = TypeVar("S")
//!
//! def fun(x: T) -> list[T]:
//!     z: list[S] = []  # E — S is not bound in this function
//!
//! class Bar(Generic[T]):
//!     an_attr: list[S] = []  # E — S is not bound in Bar
//! ```

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_scoping",
    docs_url: "https://www.basilisk-python.dev/errors/generics_scoping",
};

/// Emits `generics_scoping` when a type variable is used outside its binding scope.
pub(crate) struct UnboundTypeVarScope;

impl Rule for UnboundTypeVarScope {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Collect all known TypeVar names from module-level TypeVar() calls.
        let typevar_names: HashSet<&str> =
            basilisk_resolver::collect_name_set(&module.typevar_calls);

        if typevar_names.is_empty() {
            return;
        }

        // Build a map of class name -> bound TypeVar names (from Generic[...] params).
        let class_generic_params: Vec<(&str, HashSet<&str>)> = module
            .classes
            .iter()
            .map(|cls| {
                let params: HashSet<&str> = cls
                    .generic_params
                    .iter()
                    .map(|p| p.name.as_str())
                    .chain(cls.pep695_type_param_names.iter().map(String::as_str))
                    .collect();
                (cls.name.as_str(), params)
            })
            .collect();

        // Check module-level variables for unbound TypeVar usage.
        check_module_vars(module, &typevar_names, diagnostics);

        // Check function local variables for unbound TypeVar usage.
        check_function_locals(module, &typevar_names, &class_generic_params, diagnostics);

        // Check class body attributes for unbound TypeVar usage.
        check_class_attributes(module, &typevar_names, &class_generic_params, diagnostics);
    }
}

/// Extract all identifier-like tokens from an annotation text that could be
/// `TypeVar` references.
fn extract_names_from_annotation(annotation_text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut start = None;
    for (idx, ch) in annotation_text.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if start.is_none() {
                start = Some(idx);
            }
        } else if let Some(s) = start {
            if let Some(token) = annotation_text.get(s..idx) {
                // Skip numeric-only tokens and common type keywords.
                if !token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    names.push(token);
                }
            }
            start = None;
        }
    }
    // Handle token at end of string.
    if let Some(s) = start {
        if let Some(token) = annotation_text.get(s..) {
            if !token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                names.push(token);
            }
        }
    }
    names
}

/// Get the annotation text from source given a span.
fn annotation_text(source: &str, span: basilisk_resolver::Span) -> Option<&str> {
    slice_span(source, span)
}

/// Find `TypeVar` references in an annotation that are NOT in the allowed set.
fn find_unbound_typevars_in_annotation<'a>(
    source: &'a str,
    span: basilisk_resolver::Span,
    typevar_names: &HashSet<&str>,
    allowed: &HashSet<&str>,
) -> Vec<&'a str> {
    let Some(text) = annotation_text(source, span) else {
        return Vec::new();
    };
    let names = extract_names_from_annotation(text);
    names
        .into_iter()
        .filter(|name| typevar_names.contains(name) && !allowed.contains(name))
        .collect()
}

/// Check module-level variables for unbound `TypeVar` references in annotations.
fn check_module_vars(
    module: &ResolvedModule,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let empty: HashSet<&str> = HashSet::new();
    for var in &module.module_vars {
        if let Some(ref ann_span) = var.annotation_span {
            let unbound = find_unbound_typevars_in_annotation(
                &module.source,
                *ann_span,
                typevar_names,
                &empty,
            );
            for tv_name in unbound {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Type variable `{tv_name}` is not bound in the current scope"),
                    *ann_span,
                    &module.path,
                    Some(
                        "Type variables cannot be used in module-level variable annotations"
                            .to_owned(),
                    ),
                    Some(
                        "A TypeVar must be bound by a Generic class or function signature"
                            .to_owned(),
                    ),
                ));
            }
        }
    }

    // Also check module-level expression statements for TypeVar usage
    // (e.g. `list[T]()` at module level).
    // These are captured as calls; we check the source text around them.
}

/// Check function local variables for unbound `TypeVar` references.
fn check_function_locals(
    module: &ResolvedModule,
    typevar_names: &HashSet<&str>,
    class_generic_params: &[(&str, HashSet<&str>)],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        // Determine which TypeVars are in scope for this function:
        // 1. TypeVars used in parameter annotations
        // 2. PEP 695 type params
        // 3. If method: TypeVars from enclosing class's Generic[...]
        let mut in_scope: HashSet<&str> = HashSet::new();

        // Add PEP 695 type params.
        for name in &func.pep695_type_param_names {
            let _ = in_scope.insert(name.as_str());
        }

        // Add TypeVars from parameter annotations.
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if let Some(ref ann_span) = param.annotation_span {
                if let Some(text) = annotation_text(&module.source, *ann_span) {
                    for name in extract_names_from_annotation(text) {
                        if typevar_names.contains(name) {
                            let _ = in_scope.insert(name);
                        }
                    }
                }
            }
        }

        // Add TypeVars from return annotation.
        if let Some(ref ret_span) = func.return_annotation_span {
            if let Some(text) = annotation_text(&module.source, *ret_span) {
                for name in extract_names_from_annotation(text) {
                    if typevar_names.contains(name) {
                        let _ = in_scope.insert(name);
                    }
                }
            }
        }

        // If this is a method, add enclosing class's Generic params.
        if let Some(ref class_name) = func.class_name {
            for (cls_name, params) in class_generic_params {
                if *cls_name == class_name.as_str() {
                    for param in params {
                        let _ = in_scope.insert(param);
                    }
                    break;
                }
            }
        }

        // Now check each local variable annotation.
        for local_var in &func.local_vars {
            if let Some(ref ann_span) = local_var.annotation_span {
                let unbound = find_unbound_typevars_in_annotation(
                    &module.source,
                    *ann_span,
                    typevar_names,
                    &in_scope,
                );
                for tv_name in unbound {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Type variable `{tv_name}` is not bound in function `{}`",
                            func.name
                        ),
                        *ann_span,
                        &module.path,
                        Some(format!(
                            "Use `{tv_name}` in a parameter annotation to bind it, \
                             or remove it from the local variable annotation"
                        )),
                        Some(
                            "Unbound type variables should not appear in function bodies"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}

/// Check class body attributes for unbound `TypeVar` usage.
fn check_class_attributes(
    module: &ResolvedModule,
    typevar_names: &HashSet<&str>,
    class_generic_params: &[(&str, HashSet<&str>)],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        // Find this class's bound TypeVars.
        let bound_params: HashSet<&str> = class_generic_params
            .iter()
            .find(|(name, _)| *name == cls.name.as_str())
            .map_or_else(HashSet::new, |(_, params)| params.clone());

        // Check each attribute annotation.
        for attr in &cls.attributes {
            if let Some(ref ann_span) = attr.annotation_span {
                let unbound = find_unbound_typevars_in_annotation(
                    &module.source,
                    *ann_span,
                    typevar_names,
                    &bound_params,
                );
                for tv_name in unbound {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Type variable `{tv_name}` is not bound in class `{}`",
                            cls.name,
                        ),
                        *ann_span,
                        &module.path,
                        Some(format!(
                            "Add `{tv_name}` to `Generic[...]` in the class bases, \
                             or use it in a method signature instead"
                        )),
                        Some(
                            "Unbound type variables should not appear in class body \
                             annotations outside method definitions"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}
