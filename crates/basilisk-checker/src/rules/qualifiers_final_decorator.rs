//! Implements [`qualifiers_final_decorator`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! `qualifiers_final_decorator`: `@final` decorator violations.
//!
//! Three violations are detected:
//!
//! 1. **Inheriting from a `@final` class** — a class decorated with `@final`
//!    cannot be subclassed.
//!
//! 2. **`@final` on a non-method function** — `@final` is only valid on
//!    methods defined inside a class body, not on module-level functions.
//!
//! 3. **Overriding a `@final` method** — a method decorated with `@final`
//!    in a base class cannot be overridden in a subclass.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diag_help_note, error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_final_decorator",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_final_decorator",
};

/// Returns `true` if the decorator name is `final` or `typing.final`.
fn is_final_decorator(d: &str) -> bool {
    d == "final" || d.rsplit('.').next() == Some("final")
}

/// Emits `qualifiers_final_decorator` for `@final` decorator violations.
pub(crate) struct FinalViolation;

impl Rule for FinalViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build a map of class name → ClassInfo for fast lookup.
        let class_map = super::shared::class_name_map(&module.classes);

        // Build a map of (class_name, method_name) → FunctionInfo for span lookup.
        let method_map: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|f| {
                f.class_name
                    .as_deref()
                    .map(|cls| ((cls, f.name.as_str()), f))
            })
            .collect();

        // Check 1: Inheriting from a @final class.
        for child in &module.classes {
            for base_name in &child.bases {
                if let Some(base_cls) = class_map.get(base_name.as_str()) {
                    if base_cls.is_final {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!("Cannot inherit from final class `{}`", base_cls.name),
                            child.name_span,
                            &module.path,
                            Some(format!(
                                "Remove `{}` from the base classes of `{}`",
                                base_cls.name, child.name
                            )),
                            Some("`@final` (PEP 591) prohibits subclassing".to_owned()),
                        ));
                    }
                }
            }
        }

        // Check 2: @final on a module-level (non-method) function.
        for func in &module.functions {
            if func.class_name.is_none() && func.decorators.iter().any(|d| is_final_decorator(d)) {
                diagnostics.push(error_diag_help_note(
                    CODE.clone(),
                    format!(
                        "`@final` is not allowed on non-method function `{}`",
                        func.name
                    ),
                    func.name_span,
                    &module.path,
                    "`@final` may only be applied to methods inside a class body".to_owned(),
                    "PEP 591: `@final` on a non-method function is an error",
                ));
            }
        }

        // Check 3: Overriding a @final method in a derived class.
        //
        // For each child class, collect all @final method names from each same-module
        // base class, then flag any child method that shadows a @final base method.
        for child in &module.classes {
            // Accumulate all @final method names from all same-module bases.
            let mut final_base_methods: HashSet<&str> = HashSet::new();
            for base_name in &child.bases {
                if let Some(base_cls) = class_map.get(base_name.as_str()) {
                    for (method_name, decs) in &base_cls.method_decorators {
                        if decs.iter().any(|d| is_final_decorator(d)) {
                            let _ = final_base_methods.insert(method_name.as_str());
                        }
                    }
                }
                // Bases imported from a sibling module (e.g. a `.pyi` stub) carry
                // their `@final` methods in `imported_final_methods`.
                if let Some(methods) = module.imported_final_methods.get(base_name.as_str()) {
                    for method_name in methods {
                        let _ = final_base_methods.insert(method_name.as_str());
                    }
                }
            }

            // Emit an error for each child method that overrides a @final base method.
            // Deduplicate method_names to avoid multiple errors for overloaded methods.
            let mut seen_methods: HashSet<&str> = HashSet::new();
            for method_name in &child.method_names {
                if !seen_methods.insert(method_name.as_str()) {
                    continue;
                }
                if final_base_methods.contains(method_name.as_str()) {
                    // Look up the span from module.functions.
                    let span = method_map
                        .get(&(child.name.as_str(), method_name.as_str()))
                        .map_or(child.name_span, |f| f.name_span);
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Method `{method_name}` in `{}` overrides a `@final` method",
                            child.name
                        ),
                        span,
                        &module.path,
                        Some(format!(
                            "Remove or rename `{method_name}` — it overrides a `@final` method in a base class"
                        )),
                        Some(
                            "`@final` (PEP 591) prohibits overriding this method".to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}
