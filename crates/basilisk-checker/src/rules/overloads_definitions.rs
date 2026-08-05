//! Implements [`overloads_definitions`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `overloads_definitions`: Missing `@overload` implementation.
//!
//! When a function name is defined multiple times and every definition carries
//! the `@overload` decorator, there is no concrete implementation body.
//! Python's `typing.overload` protocol requires exactly one implementation
//! function without `@overload`.
//!
//! This rule fires once per overload group that lacks a plain implementation.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::guards::is_protocol_class;
use super::shared::overload_decorated;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_definitions",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_definitions",
};

/// Emits `overloads_definitions` when a set of `@overload` functions has no matching
/// implementation (a same-named function without `@overload`).
// Implements [TYPEINF-FUNC-OVERLOADS] — an `@overload` group in a regular module
// requires a concrete implementation signature.
pub(crate) struct MissingOverloadImpl;

impl Rule for MissingOverloadImpl {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Whether a decorator IS `typing.overload` is a binding question,
        // answered by the resolver's tables ([#380]) — never by its spelling.
        let Some(resolver) = types.annotations() else {
            return;
        };
        // Build a set of Protocol class names so we can exempt their methods.
        let protocol_classes: std::collections::HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| is_protocol_class(cls))
            .map(|cls| cls.name.as_str())
            .collect();

        // Group functions by (class_name, function_name) to handle overloads correctly
        // across different classes that may have the same method name.
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups
                .entry((func.class_name.as_deref(), &func.name))
                .or_default()
                .push(func);
        }

        for ((class_name, name), funcs) in &groups {
            let overloaded: Vec<&&FunctionInfo> = funcs
                .iter()
                .filter(|f| overload_decorated(resolver, &f.decorators))
                .collect();

            // No @overload decorators in this group — nothing to check.
            if overloaded.is_empty() {
                continue;
            }

            let non_overloaded: Vec<&&FunctionInfo> = funcs
                .iter()
                .filter(|f| !overload_decorated(resolver, &f.decorators))
                .collect();

            // Case 1: ALL definitions carry @overload (no implementation).
            if non_overloaded.is_empty() {
                let is_protocol = class_name.is_some_and(|cls| protocol_classes.contains(cls));

                // A lone `@overload` is invalid in any context: the spec requires
                // at least two `@overload`-decorated definitions. Stubs/protocols
                // are exempt only from the *implementation* requirement below,
                // not from the "at least two" rule. (Protocol single
                // declarations are tolerated to avoid noise on partial code.)
                if overloaded.len() == 1 {
                    if !is_protocol {
                        if let Some(first) = overloaded.first() {
                            diagnostics.push(make_single_overload_diagnostic(
                                first,
                                name,
                                &module.path,
                            ));
                        }
                    }
                    continue;
                }

                // 2+ overloads with no implementation. Stub files (`.pyi`)
                // and Protocols legitimately omit it.
                let is_stub = std::path::Path::new(&module.path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pyi"));

                if !is_protocol && !is_stub {
                    if let Some(first) = overloaded.first() {
                        diagnostics.push(make_diagnostic(
                            first,
                            name,
                            overloaded.len(),
                            &module.path,
                        ));
                    }
                }
                continue;
            }

            // Case 2: Has at least one @overload AND at least one non-overload
            // implementation, but only exactly 1 @overload — invalid because at
            // least two @overload signatures are required.
            if overloaded.len() == 1 {
                if let Some(first) = overloaded.first() {
                    diagnostics.push(make_single_overload_diagnostic(first, name, &module.path));
                }
            }

            // Case 3: 2+ @overload + at least 1 non-overload implementation — valid.
        }
    }
}

fn make_diagnostic(
    first_func: &FunctionInfo,
    name: &str,
    overload_count: usize,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{}` has {} `@overload` signature{} but no implementation",
            name,
            overload_count,
            if overload_count == 1 { "" } else { "s" },
        ),
        first_func.name_span,
        path,
        Some(format!(
            "Add an implementation function `def {name}(...)` without `@overload`"
        )),
        Some(
            "`@overload` signatures are type-only; a concrete implementation is required at runtime"
                .to_owned(),
        ),
    )
}

fn make_single_overload_diagnostic(
    first_func: &FunctionInfo,
    name: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{name}` has only 1 `@overload` signature — at least two are required",
        ),
        first_func.name_span,
        path,
        Some(format!(
            "Add at least one more `@overload`-decorated signature for `{name}`"
        )),
        Some(
            "Python's `@overload` protocol requires at least two type signatures before the implementation"
                .to_owned(),
        ),
    )
}
