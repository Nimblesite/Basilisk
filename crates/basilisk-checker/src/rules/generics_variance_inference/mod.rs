//! generics_variance_inference: `TypeVar` scoping violation.
//!
//! Detects uses of `TypeVar` instances outside their valid scope:
//!
//! 1. A nested class inside a generic class using the outer class's `TypeVar`
//!    in its base classes or body (the outer class's type params don't cover
//!    the inner class scope).
//! 2. A class nested inside a generic function re-using the function's `TypeVar`
//!    in `Generic[...]`.
//! 3. A `TypeVar` used in a module-level expression (subscript call like `list[T]()`).
//! 4. A method call on a generic class instance where the argument type does not
//!    match the substituted `TypeVar` type (e.g., `a: MyClass[int]`, calling
//!    `a.meth('str')` when `meth` expects `T` which is bound to `int`).
//!
//! Per PEP 484: "A generic class nested in another generic class cannot use
//! the same type variables."

mod check;
mod collect;
mod types;
mod utils;
mod variance;
mod variance_check;

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;
use crate::rules::shared::contains_typevar_reference;

use check::{check_generic_constructor_calls, check_generic_instance_method_calls};
use types::ScopeInfo;
use utils::{
    collect_full_signature, compute_triple_quote_mask, extract_pep695_type_params,
    extract_typevars_from_function_sig, extract_typevars_from_generic_base, is_simple_assignment,
    leading_indent, signature_end_line, span_for_line,
};
use variance::check_variance_assignments;

const CODE: ErrorCode = ErrorCode {
    code: "generics_variance_inference",
    docs_url: "https://www.basilisk-python.dev/errors/generics_variance_inference",
};

/// Emits generics_variance_inference for `TypeVar` scoping violations.
pub(crate) struct TypeVarScopeViolation;

impl Rule for TypeVarScopeViolation {
    #[expect(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "TypeVar scoping requires many interleaved checks"
    )]
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let all_typevars: HashSet<String> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.clone())
            .collect();

        if all_typevars.is_empty() {
            return;
        }

        let lines: Vec<&str> = module.source.lines().collect();
        let triple_quote_mask = compute_triple_quote_mask(&lines);

        // Track scope stack: each entry has (indent, bound_typevars, is_class).
        let mut scope_stack: Vec<ScopeInfo> = Vec::new();

        // When > 0, we are inside a multi-line function signature and should
        // skip lines until this index (inclusive, 0-based).
        let mut skip_until_line: usize = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // Skip empty lines, comments, and pure string lines.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Skip lines inside triple-quoted strings (docstrings, multi-line strings).
            if triple_quote_mask.get(line_idx).copied().unwrap_or(false) {
                continue;
            }

            // Skip continuation lines of multi-line function signatures.
            if line_idx < skip_until_line {
                continue;
            }

            let indent = leading_indent(line);

            // Pop scopes that are no longer active (indentation decreased).
            while let Some(top) = scope_stack.last() {
                if indent <= top.indent {
                    let _ = scope_stack.pop();
                } else {
                    break;
                }
            }

            // Detect class definitions.
            if trimmed.starts_with("class ") {
                let mut bound_tvs = extract_typevars_from_generic_base(trimmed);
                // PEP 695 type params (class Foo[T, S]:) also bind TypeVars
                bound_tvs.extend(extract_pep695_type_params(trimmed));

                // Check: does this class's base reference a TypeVar from an outer scope?
                let outer_bound: HashSet<String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter().cloned())
                    .collect();

                if !outer_bound.is_empty() {
                    // Check if any outer TypeVar is used in the base classes.
                    // Extract the base class portion: everything between `(` and `)`.
                    if let Some(paren_start) = trimmed.find('(') {
                        if let Some(paren_end) = trimmed.rfind(')') {
                            let bases_text = &trimmed[paren_start + 1..paren_end];
                            for typevar_name in &outer_bound {
                                if contains_typevar_reference(bases_text, typevar_name) {
                                    diagnostics.push(error_diagnostic_owned(
                                        CODE.clone(),
                                        format!(
                                            "TypeVar `{typevar_name}` from outer scope \
                                             cannot be used in nested class definition"
                                        ),
                                        span_for_line(&module.source, line_number),
                                        &module.path,
                                        Some(
                                            "Use a different TypeVar for the inner class, \
                                             or restructure to avoid nesting"
                                                .to_owned(),
                                        ),
                                        Some(
                                            "PEP 484: the scope of type variables of the \
                                             outer class doesn't cover the inner one"
                                                .to_owned(),
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }

                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: true,
                });
                continue;
            }

            // Detect function definitions. For multi-line signatures, collect
            // the full signature text so TypeVars in parameter annotations are
            // correctly recognised as bound, and skip continuation lines.
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let full_sig = collect_full_signature(&lines, line_idx);
                let bound_tvs = extract_typevars_from_function_sig(&full_sig, &all_typevars);
                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: false,
                });
                // Mark continuation lines so they are not analysed as standalone code.
                let sig_end = signature_end_line(&lines, line_idx);
                if sig_end > line_idx {
                    skip_until_line = sig_end + 1;
                }
                continue;
            }

            // For lines inside a nested class body (inner class of a generic class),
            // check if annotations reference outer class TypeVars.
            // This only applies when there are at least 2 class scopes on the stack
            // (outer generic class + inner class).
            {
                let class_scopes: Vec<&ScopeInfo> =
                    scope_stack.iter().filter(|scope| scope.is_class).collect();

                if class_scopes.len() >= 2 {
                    // Collect TypeVars from all outer class scopes (all but the last).
                    let outer_class_tvs: HashSet<String> = class_scopes
                        .get(..class_scopes.len().saturating_sub(1))
                        .unwrap_or_default()
                        .iter()
                        .flat_map(|scope| scope.bound_typevars.iter().cloned())
                        .collect();

                    let innermost_tvs = &class_scopes
                        .last()
                        .map_or_else(HashSet::new, |scope| scope.bound_typevars.clone());

                    let forbidden_tvs: HashSet<&String> = outer_class_tvs
                        .iter()
                        .filter(|tv| !innermost_tvs.contains(*tv))
                        .collect();

                    if !forbidden_tvs.is_empty() && trimmed.contains(':') {
                        // This is an annotation line — check for forbidden TypeVar refs.
                        let annotation_part =
                            trimmed.split_once(':').map_or(trimmed, |(_, rhs)| rhs);
                        for typevar_name in &forbidden_tvs {
                            if contains_typevar_reference(annotation_part, typevar_name) {
                                diagnostics.push(error_diagnostic_owned(
                                    CODE.clone(),
                                    format!(
                                        "TypeVar `{typevar_name}` from outer class \
                                         cannot be used in nested class body"
                                    ),
                                    span_for_line(&module.source, line_number),
                                    &module.path,
                                    Some("Use a different TypeVar for the inner class".to_owned()),
                                    Some(
                                        "PEP 484: the scope of type variables of the \
                                         outer class doesn't cover the inner one"
                                            .to_owned(),
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            // Check for unbound TypeVar usage in function bodies and class
            // attribute annotations. A TypeVar is "unbound" if no enclosing
            // scope binds it (via Generic[...] or function signature).
            if !scope_stack.is_empty() && trimmed.contains(':') {
                // Collect all TypeVars bound by any enclosing scope.
                let all_bound: HashSet<&String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter())
                    .collect();

                let annotation_part = trimmed.split_once(':').map_or(trimmed, |(_, rhs)| rhs);
                let before_comment = annotation_part
                    .split_once('#')
                    .map_or(annotation_part, |(code, _)| code);

                for typevar_name in &all_typevars {
                    if !all_bound.contains(typevar_name)
                        && contains_typevar_reference(before_comment, typevar_name)
                    {
                        // Skip if this is a function def line (already handles its own
                        // TypeVars via scope creation).
                        let is_def_line =
                            trimmed.starts_with("def ") || trimmed.starts_with("async def ");
                        if !is_def_line {
                            diagnostics.push(error_diagnostic_owned(
                                CODE.clone(),
                                format!(
                                    "TypeVar `{typevar_name}` is not bound in \
                                     this scope"
                                ),
                                span_for_line(&module.source, line_number),
                                &module.path,
                                Some(
                                    "TypeVars can only be used where they are \
                                     bound by a Generic[...] base or function signature"
                                        .to_owned(),
                                ),
                                Some(
                                    "PEP 484: unbound type variables should not \
                                     appear in function or class bodies"
                                        .to_owned(),
                                ),
                            ));
                        }
                    }
                }
            }

            // TypeAlias inside a class body: class TypeVars are not in scope
            // for type alias definitions. `alias: TypeAlias = list[T]` is invalid
            // when T comes from the enclosing class's Generic[T].
            //
            // A `TypeAliasType(...)` call is excluded: unlike the `TypeAlias`
            // annotation it does not open a new scope, so it may legitimately
            // capture the enclosing class's TypeVars when `type_params` is omitted.
            {
                let class_scopes: Vec<&ScopeInfo> =
                    scope_stack.iter().filter(|scope| scope.is_class).collect();

                if class_scopes.len() == 1
                    && trimmed.contains("TypeAlias")
                    && !trimmed.contains("TypeAliasType")
                {
                    let Some(enclosing_tvs) = class_scopes.first().map(|s| &s.bound_typevars)
                    else {
                        continue;
                    };
                    if !enclosing_tvs.is_empty() {
                        // Check the RHS of the TypeAlias assignment for TypeVar refs.
                        let rhs_part = trimmed.split_once('=').map_or("", |(_, rhs)| rhs);
                        for typevar_name in enclosing_tvs {
                            if contains_typevar_reference(rhs_part, typevar_name) {
                                diagnostics.push(error_diagnostic_owned(
                                    CODE.clone(),
                                    format!(
                                        "TypeVar `{typevar_name}` from enclosing class \
                                         is not accessible in a TypeAlias definition"
                                    ),
                                    span_for_line(&module.source, line_number),
                                    &module.path,
                                    Some(
                                        "Type aliases in class bodies cannot reference \
                                         the class's type parameters"
                                            .to_owned(),
                                    ),
                                    Some(
                                        "PEP 484: TypeAlias creates its own scope and \
                                         cannot capture class-level TypeVars"
                                            .to_owned(),
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            // Module-level (indent == 0, no enclosing scope): check for TypeVar
            // subscript expressions like `list[T]()`.
            // Per PEP 484/613, module-level assignments using TypeVars are valid
            // implicit type aliases (e.g. `Vector = list[T]`), so we only flag
            // non-assignment expressions.
            if indent == 0 && scope_stack.is_empty() {
                let dominated_by_other = trimmed.starts_with("class ")
                    || trimmed.starts_with("def ")
                    || trimmed.starts_with("async def ")
                    || trimmed.starts_with("import ")
                    || trimmed.starts_with("from ")
                    || trimmed.starts_with('@');

                if !dominated_by_other {
                    let before_comment = trimmed.split_once('#').map_or(trimmed, |(code, _)| code);

                    // Module-level assignments with TypeVars are implicit type aliases
                    // (PEP 484): `MyAlias = list[T]` is valid. Also skip TypeVar
                    // definitions, ParamSpec, TypeVarTuple, and TypeAlias annotations.
                    let is_type_alias_or_def = is_simple_assignment(before_comment)
                        || before_comment.contains("TypeVar")
                        || before_comment.contains("ParamSpec")
                        || before_comment.contains("TypeVarTuple")
                        || before_comment.contains("TypeAlias");

                    if !is_type_alias_or_def {
                        for typevar_name in &all_typevars {
                            if contains_typevar_reference(before_comment, typevar_name) {
                                diagnostics.push(error_diagnostic_owned(
                                    CODE.clone(),
                                    format!(
                                        "TypeVar `{typevar_name}` is not bound in \
                                         this scope"
                                    ),
                                    span_for_line(&module.source, line_number),
                                    &module.path,
                                    Some(
                                        "TypeVars can only be used inside generic \
                                         functions or classes that bind them"
                                            .to_owned(),
                                    ),
                                    Some(
                                        "PEP 484: unbound type variables should not \
                                         appear at module scope"
                                            .to_owned(),
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Check generic class instance method calls for TypeVar substitution violations.
        check_generic_instance_method_calls(
            &module.source,
            &all_typevars,
            diagnostics,
            &module.path,
        );

        // Check variance-related assignment violations (PEP 695 + infer_variance).
        check_variance_assignments(module, diagnostics);

        // Check constructor calls with TypeVar default propagation.
        check_generic_constructor_calls(module, diagnostics);
    }
}
