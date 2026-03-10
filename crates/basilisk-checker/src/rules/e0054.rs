//! BSK-E0054: `Final` type qualifier annotation violations.
//!
//! Detects violations of PEP 591's rules for the `Final` qualifier, beyond the
//! positional errors handled by E0044. Specifically:
//!
//! 1. **Class attribute `Final` without init** — `ID2: Final` / `ID3: Final[int]`
//!    in a class body without an initializer and not assigned in `__init__`.
//!
//! 2. **Instance `Final` outside `__init__`** — `self.id3: Final = 1` in a method
//!    other than `__init__`.
//!
//! 3. **Re-assignment to already-initialized Final** — `self.ID5 = 0` when
//!    `ID5: Final[int] = 0` is already given a value in the class body.
//!
//! 4. **Modification of Final class attribute** — `self.ID7 = 0` / `self.ID7 += 1`
//!    when `ID7` is declared `Final` in the class body.
//!
//! 5. **Module-level Final re-assignment** — `RATE = 300` after `RATE: Final = 3000`.
//!
//! 6. **Class attribute re-assignment** — `ClassB.DEFAULT_ID = 0` when `DEFAULT_ID`
//!    is declared `Final` in `ClassB`.
//!
//! 7. **Subclass override of Final** — `BORDER_WIDTH = 2.5` in a subclass when the
//!    parent declares `BORDER_WIDTH: Final = 2.5`.
//!
//! 8. **Function-local Final modification** — `x += 1` when `x: Final = 3`, or
//!    walrus/for/with/tuple-unpack on a `Final` variable.
//!
//! 9. **Global Final modification** — `global ID1; ID1 = 2` inside a function
//!    when `ID1` is a module-level `Final`.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{FinalViolationKind, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0054",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0054",
};

fn make_diagnostic(message: String, span: Span, path: &str, help: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(help.to_owned()),
        note: Some(
            "PEP 591: `Final` names may only be assigned once at declaration time".to_owned(),
        ),
    }
}

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let sp = span?;
    source.get(sp.start as usize..sp.end as usize)
}

fn annotation_text_is_final(text: &str) -> bool {
    let t = text.trim();
    t == "Final"
        || t.starts_with("Final[")
        || t == "typing.Final"
        || t.starts_with("typing.Final[")
        || t.starts_with("ClassVar[Final")
        || t.starts_with("ClassVar[typing.Final")
}

fn collect_module_final_names(module: &ResolvedModule) -> HashSet<String> {
    module
        .module_vars
        .iter()
        .filter(|v| {
            v.has_annotation
                && span_text(&module.source, v.annotation_span)
                    .is_some_and(annotation_text_is_final)
        })
        .map(|v| v.name.clone())
        .collect()
}

fn collect_class_final_attr_map(module: &ResolvedModule) -> HashMap<String, HashSet<String>> {
    module
        .classes
        .iter()
        .map(|cls| {
            let finals: HashSet<String> = cls
                .attributes
                .iter()
                .filter(|a| {
                    a.has_annotation
                        && span_text(&module.source, a.annotation_span)
                            .is_some_and(annotation_text_is_final)
                })
                .map(|a| a.name.clone())
                .collect();
            (cls.name.clone(), finals)
        })
        .collect()
}

/// Emits BSK-E0054 for `Final` annotation violations collected during resolution.
pub(crate) struct FinalAnnotationViolation;

impl Rule for FinalAnnotationViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let path = &module.path;
        let module_final_names = collect_module_final_names(module);
        let class_final_map = collect_class_final_attr_map(module);
        check_final_violations(module, &module_final_names, path, diagnostics);
        check_module_bare_assignments(module, &module_final_names, path, diagnostics);
        check_class_attr_assignments(module, &class_final_map, path, diagnostics);
    }
}

fn check_final_violations(
    module: &ResolvedModule,
    module_final_names: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for violation in &module.final_violations {
        let span = violation.span;
        let name = &violation.name;
        match &violation.kind {
            FinalViolationKind::ClassFinalWithoutInit => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`{name}` is annotated `Final` in the class body but has no \
                         initializer and is not unconditionally assigned in `__init__`"
                    ),
                    span,
                    path,
                    "Either add an initializer (`ID: Final = value`) or assign \
                     unconditionally in `__init__`",
                ));
            }
            FinalViolationKind::InstanceFinalOutsideInit => {
                diagnostics.push(make_diagnostic(
                    format!("`self.{name}: Final` annotation is only allowed inside `__init__`"),
                    span,
                    path,
                    "Move `Final` instance attribute declarations to `__init__`",
                ));
            }
            FinalViolationKind::InstanceReassignAlreadyInitialized => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Cannot assign to `self.{name}` — it is already initialized as \
                         `Final` in the class body"
                    ),
                    span,
                    path,
                    "Remove the assignment or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::InstanceModifyFinal => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Cannot assign to `self.{name}` — it is declared `Final` in \
                         the class body"
                    ),
                    span,
                    path,
                    "Remove the assignment or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::SubclassOverrideFinal => {
                diagnostics.push(make_diagnostic(
                    format!("Cannot override `{name}` — it is declared `Final` in a base class"),
                    span,
                    path,
                    "Remove the override or rename the attribute",
                ));
            }
            FinalViolationKind::FunctionLocalFinalModification => {
                diagnostics.push(make_diagnostic(
                    format!("Cannot modify `{name}` — it is declared `Final`"),
                    span,
                    path,
                    "Remove the modification or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::GlobalFinalModification => {
                if module_final_names.contains(name.as_str()) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Cannot assign to global `{name}` — it is declared `Final` \
                             at module level"
                        ),
                        span,
                        path,
                        "Remove the assignment or remove the `Final` annotation",
                    ));
                }
            }
            FinalViolationKind::ModuleLevelReassignment
            | FinalViolationKind::ClassAttributeReassignment => {}
        }
    }
}

fn check_module_bare_assignments(
    module: &ResolvedModule,
    module_final_names: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bare in &module.module_bare_assignments {
        if module_final_names.contains(&bare.name) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Cannot re-assign `{}` — it is declared `Final` at module level",
                    bare.name
                ),
                bare.name_span,
                path,
                "Remove the re-assignment or remove the `Final` annotation",
            ));
        } else if module.imported_final_names.contains(&bare.name) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Cannot re-assign `{}` — it is declared `Final` in an imported module",
                    bare.name
                ),
                bare.name_span,
                path,
                "Remove the re-assignment or remove the import",
            ));
        }
    }
}

fn check_class_attr_assignments(
    module: &ResolvedModule,
    class_final_map: &HashMap<String, HashSet<String>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Build a map from module-level variable name → annotated type name (for instance vars).
    let var_type_map: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter_map(|v| {
            let span = v.annotation_span?;
            let ann = module
                .source
                .get(span.start as usize..span.end as usize)?
                .trim();
            Some((v.name.as_str(), ann))
        })
        .collect();

    // Build a RHS-based map: variable name → class name inferred from constructor call.
    // Handles `d = D(...)` without an explicit annotation.
    let source = &module.source;
    let rhs_instance_map: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter_map(|v| {
            let rhs_span = v.rhs_span?;
            let rhs = source.get(rhs_span.start as usize..rhs_span.end as usize)?;
            let callee = rhs.split(['(', '[']).next()?.trim();
            if callee.is_empty() {
                return None;
            }
            let callee = callee.rsplit('.').next().unwrap_or(callee);
            if class_final_map.contains_key(callee) {
                Some((v.name.as_str(), callee))
            } else {
                None
            }
        })
        .collect();

    for attr_assign in &module.module_attr_assignments {
        // Try direct class name lookup first (e.g., `D.final_attr = ...`).
        let finals = if let Some(f) = class_final_map.get(&attr_assign.object_name) {
            f
        } else if let Some(type_name) = var_type_map.get(attr_assign.object_name.as_str()) {
            // Fall back to annotated instance variable type lookup (e.g., `d: D = D(...); d.final_attr = ...`).
            if let Some(f) = class_final_map.get(*type_name) {
                f
            } else {
                continue;
            }
        } else if let Some(class_name) = rhs_instance_map.get(attr_assign.object_name.as_str()) {
            // Fall back to RHS-inferred instance type (e.g., `d = D(...); d.final_attr = ...`).
            if let Some(f) = class_final_map.get(*class_name) {
                f
            } else {
                continue;
            }
        } else {
            continue;
        };

        if finals.contains(&attr_assign.attr_name) {
            let class_name = class_final_map
                .keys()
                .find(|k| {
                    *k == &attr_assign.object_name
                        || var_type_map
                            .get(attr_assign.object_name.as_str())
                            .is_some_and(|t| *t == k.as_str())
                })
                .map_or(attr_assign.object_name.as_str(), String::as_str);
            diagnostics.push(make_diagnostic(
                format!(
                    "Cannot assign to `{}.{}` — it is declared `Final` in `{}`",
                    attr_assign.object_name, attr_assign.attr_name, class_name
                ),
                attr_assign.target_span,
                path,
                "Remove the assignment or remove the `Final` annotation",
            ));
        }
    }
}
