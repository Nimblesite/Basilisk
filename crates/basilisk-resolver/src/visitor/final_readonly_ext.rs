//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Final Readonly Ext visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::ClassInfo;

use super::annotations::ann_text_is_final;
use super::assigns::collect_unconditional_self_assigns;
use super::core::{source_slice_range, source_slice_span, text_range_to_span};

pub(super) fn collect_final_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
    source: &str,
) -> Vec<crate::scope::FinalViolationInfo> {
    let mut out = Vec::new();

    // Collect module-level Final names for GlobalFinalModification.
    let module_final_names: std::collections::HashSet<&str> = stmts
        .iter()
        .filter_map(|s| {
            let Stmt::AnnAssign(ann) = s else { return None };
            let Expr::Name(n) = ann.target.as_ref() else {
                return None;
            };
            let range = ann.annotation.range();
            let ann_text = source_slice_range(source, range)?;
            ann_text_is_final(ann_text).then(|| n.id.as_str())
        })
        .collect();

    // Build a class-name -> Final-attr-names map for SubclassOverrideFinal.
    let class_finals: std::collections::HashMap<&str, std::collections::HashSet<&str>> = classes
        .iter()
        .map(|cls| {
            let finals: std::collections::HashSet<&str> = cls
                .attributes
                .iter()
                .filter(|a| {
                    a.has_annotation
                        && a.annotation_span
                            .and_then(|sp| source_slice_span(source, sp))
                            .is_some_and(ann_text_is_final)
                })
                .map(|a| a.name.as_str())
                .collect();
            (cls.name.as_str(), finals)
        })
        .collect();

    // Walk class definitions for per-class violations.
    let empty_locals: std::collections::HashSet<String> = std::collections::HashSet::new();
    for stmt in stmts {
        match stmt {
            Stmt::ClassDef(cls_def) => {
                collect_class_final_violations(cls_def, &class_finals, source, &mut out);
                // Also check methods inside the class for global Final modifications.
                for body_stmt in &cls_def.body {
                    if let Stmt::FunctionDef(method) = body_stmt {
                        collect_func_final_violations(
                            method,
                            &module_final_names,
                            source,
                            &mut out,
                        );
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                collect_func_final_violations(func, &module_final_names, source, &mut out);
            }
            // Check module-level walrus operators in if/while/expr statements.
            Stmt::If(node) => {
                check_walrus_final(&node.test, &module_final_names, &empty_locals, &mut out);
            }
            Stmt::While(node) => {
                check_walrus_final(&node.test, &module_final_names, &empty_locals, &mut out);
            }
            Stmt::Expr(expr_stmt) => {
                check_walrus_final(
                    &expr_stmt.value,
                    &module_final_names,
                    &empty_locals,
                    &mut out,
                );
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    check_final_assign_target(target, &module_final_names, &empty_locals, &mut out);
                }
                check_walrus_final(&assign.value, &module_final_names, &empty_locals, &mut out);
            }
            Stmt::AugAssign(aug) => {
                check_final_assign_target(
                    aug.target.as_ref(),
                    &module_final_names,
                    &empty_locals,
                    &mut out,
                );
            }
            _ => {}
        }
    }
    out
}

/// Collect Final violations inside a class definition.
pub(super) fn collect_class_final_violations(
    cls_def: &StmtClassDef,
    class_finals: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};

    // Find parent class Final attrs for SubclassOverrideFinal.
    let base_names: Vec<&str> = cls_def
        .arguments
        .as_deref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(|base| {
                    if let Expr::Name(n) = base {
                        Some(n.id.as_str())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Collect Final attrs from all parent classes.
    let parent_finals: std::collections::HashSet<&str> = base_names
        .iter()
        .filter_map(|name| class_finals.get(name))
        .flat_map(|set| set.iter().copied())
        .collect();

    // Collect Final attrs in THIS class (annotation-only or with value).
    let mut this_final_attrs: std::collections::HashMap<&str, bool> =
        std::collections::HashMap::new();
    // key = attr name, value = has_initializer
    for body_stmt in &cls_def.body {
        let Stmt::AnnAssign(ann) = body_stmt else {
            continue;
        };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        let attr_name = n.id.as_str();
        let range = ann.annotation.range();
        let Some(ann_text) = source_slice_range(source, range) else {
            continue;
        };
        if ann_text_is_final(ann_text) {
            let has_value = ann.value.is_some();
            let _ = this_final_attrs.insert(attr_name, has_value);
        }
    }

    // Find attrs unconditionally assigned in __init__.
    let init_assigns: std::collections::HashSet<String> = cls_def
        .body
        .iter()
        .find_map(|s| {
            if let Stmt::FunctionDef(f) = s {
                if f.name.as_str() == "__init__" {
                    return Some(collect_unconditional_self_assigns(&f.body));
                }
            }
            None
        })
        .unwrap_or_default();

    // In a `@dataclass`, a non-`ClassVar` `Final` field without an inline default
    // is valid: the synthesized `__init__` assigns it as a required constructor
    // parameter. A `ClassVar[Final[...]]` is not a field, so it still needs a value.
    let is_dataclass = super::dataclass::is_dataclass_decorated(cls_def);

    // ClassFinalWithoutInit: attr has no initializer AND not in __init__ assignments.
    for (attr_name, has_value) in &this_final_attrs {
        if !has_value && !init_assigns.contains(*attr_name) {
            // Find the span of this annotation.
            for body_stmt in &cls_def.body {
                let Stmt::AnnAssign(ann) = body_stmt else {
                    continue;
                };
                let Expr::Name(n) = ann.target.as_ref() else {
                    continue;
                };
                if n.id.as_str() != *attr_name {
                    continue;
                }
                let is_dataclass_field = is_dataclass
                    && source_slice_range(source, ann.annotation.range())
                        .is_some_and(|t| !t.trim_start().starts_with("ClassVar"));
                if is_dataclass_field {
                    break;
                }
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::ClassFinalWithoutInit,
                    span: text_range_to_span(ann.range()),
                    name: (*attr_name).to_string(),
                });
                break;
            }
        }
    }

    // Walk all method bodies for instance Final violations.
    for body_stmt in &cls_def.body {
        let Stmt::FunctionDef(method) = body_stmt else {
            continue;
        };
        let is_init = method.name.as_str() == "__init__";
        for method_stmt in &method.body {
            collect_instance_final_violations(method_stmt, is_init, &this_final_attrs, source, out);
        }
    }

    collect_subclass_override_final(cls_def, &parent_finals, out);

    // Recurse into nested class definitions.
    for body_stmt in &cls_def.body {
        if let Stmt::ClassDef(nested) = body_stmt {
            collect_class_final_violations(nested, class_finals, source, out);
        }
    }
}

/// Detect a child class declaring an attr that is `Final` in a parent.
pub(super) fn collect_subclass_override_final(
    cls_def: &StmtClassDef,
    parent_finals: &std::collections::HashSet<&str>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::FinalViolationKind;
    for body_stmt in &cls_def.body {
        let attr_name = match body_stmt {
            Stmt::Assign(assign) if assign.targets.len() == 1 => {
                if let Some(Expr::Name(n)) = assign.targets.first() {
                    n.id.as_str()
                } else {
                    continue;
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Expr::Name(n) = ann.target.as_ref() {
                    n.id.as_str()
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        // Skip private name-mangled attributes.
        if attr_name.starts_with("__") && !attr_name.ends_with("__") {
            continue;
        }

        if parent_finals.contains(attr_name) {
            let span = match body_stmt {
                Stmt::Assign(assign) => text_range_to_span(assign.range()),
                Stmt::AnnAssign(ann) => text_range_to_span(ann.range()),
                _ => continue,
            };
            out.push(crate::scope::FinalViolationInfo {
                kind: FinalViolationKind::SubclassOverrideFinal,
                span,
                name: attr_name.to_string(),
            });
        }
    }
}

/// Check a single statement inside a method body for instance Final violations.
pub(super) fn collect_instance_final_violations(
    stmt: &Stmt,
    is_init: bool,
    class_final_attrs: &std::collections::HashMap<&str, bool>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    match stmt {
        Stmt::AnnAssign(ann)
            // self.x: Final = ... outside __init__
            if !is_init => {
                if let Expr::Attribute(attr) = ann.target.as_ref() {
                    let Some(ann_text) = source_slice_range(source, ann.annotation.range()) else {
                        return;
                    };
                    if ann_text_is_final(ann_text) {
                        if let Expr::Name(self_name) = attr.value.as_ref() {
                            if self_name.id == "self" {
                                out.push(FinalViolationInfo {
                                    kind: FinalViolationKind::InstanceFinalOutsideInit,
                                    span: text_range_to_span(ann.range()),
                                    name: attr.attr.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                let Expr::Attribute(attr) = target else {
                    continue;
                };
                let Expr::Name(self_name) = attr.value.as_ref() else {
                    continue;
                };
                if self_name.id != "self" {
                    continue;
                }
                let field_name = attr.attr.as_str();
                if let Some(&has_value) = class_final_attrs.get(field_name) {
                    let kind = if is_init && has_value {
                        FinalViolationKind::InstanceReassignAlreadyInitialized
                    } else if !is_init {
                        FinalViolationKind::InstanceModifyFinal
                    } else {
                        continue;
                    };
                    out.push(FinalViolationInfo {
                        kind,
                        span: text_range_to_span(assign.range()),
                        name: field_name.to_string(),
                    });
                }
            }
        }
        Stmt::AugAssign(aug) => {
            // self.X += 1 — augmented assignment to Final class attr
            let Expr::Attribute(attr) = aug.target.as_ref() else {
                return;
            };
            let Expr::Name(self_name) = attr.value.as_ref() else {
                return;
            };
            if self_name.id != "self" {
                return;
            }
            let field_name = attr.attr.as_str();
            if class_final_attrs.contains_key(field_name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::InstanceModifyFinal,
                    span: text_range_to_span(aug.range()),
                    name: field_name.to_string(),
                });
            }
        }
        _ => {}
    }
}

/// Collect the names of attributes unconditionally assigned via `self.X = ...` in
/// the top-level statements of a function body (i.e., not inside if/for/while/try).
pub(super) fn collect_func_final_violations(
    func: &StmtFunctionDef,
    module_final_names: &std::collections::HashSet<&str>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    // Find `global X` declarations to know which names are global Final.
    let global_final_names: std::collections::HashSet<&str> = func
        .body
        .iter()
        .filter_map(|s| {
            if let Stmt::Global(g) = s {
                Some(g.names.iter().filter_map(|name| {
                    if module_final_names.contains(name.as_str()) {
                        Some(name.as_str())
                    } else {
                        None
                    }
                }))
            } else {
                None
            }
        })
        .flatten()
        .collect();

    // Collect function-local Final variables (x: Final = ...) as we scan.
    let mut local_finals: std::collections::HashSet<String> = std::collections::HashSet::new();

    for stmt in &func.body {
        collect_func_stmt_final_violations(
            stmt,
            &global_final_names,
            &mut local_finals,
            source,
            out,
        );
    }
}

/// Process a single statement inside a function for Final violations.
pub(super) fn collect_func_stmt_final_violations(
    stmt: &Stmt,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &mut std::collections::HashSet<String>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    match stmt {
        Stmt::AnnAssign(ann) => {
            // Register x: Final = ... as a local Final.
            if let Expr::Name(n) = ann.target.as_ref() {
                let range = ann.annotation.range();
                if let Some(ann_text) = source_slice_range(source, range) {
                    if ann_text_is_final(ann_text) {
                        let _ = local_finals.insert(n.id.to_string());
                    }
                }
            }
        }
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                check_final_assign_target(target, global_finals, local_finals, out);
            }
            // Also check for walrus operators in the RHS: `a = (x := 4)`.
            check_walrus_final(&assign.value, global_finals, local_finals, out);
        }
        Stmt::AugAssign(aug) => {
            check_final_assign_target(aug.target.as_ref(), global_finals, local_finals, out);
        }
        Stmt::For(for_stmt) => {
            check_final_assign_target(for_stmt.target.as_ref(), global_finals, local_finals, out);
        }
        Stmt::With(with_stmt) => {
            for item in &with_stmt.items {
                if let Some(opt_var) = &item.optional_vars {
                    check_final_assign_target(opt_var.as_ref(), global_finals, local_finals, out);
                }
            }
        }
        Stmt::Expr(expr_stmt) => {
            check_walrus_final(expr_stmt.value.as_ref(), global_finals, local_finals, out);
        }
        _ => {}
    }
}

/// Check if an assign target is a Final name and emit violations if so.
pub(super) fn check_final_assign_target(
    target: &Expr,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &std::collections::HashSet<String>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    match target {
        Expr::Name(n) => {
            let name = n.id.as_str();
            if global_finals.contains(name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::GlobalFinalModification,
                    span: text_range_to_span(n.range()),
                    name: name.to_string(),
                });
            } else if local_finals.contains(name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::FunctionLocalFinalModification,
                    span: text_range_to_span(n.range()),
                    name: name.to_string(),
                });
            }
        }
        Expr::Tuple(tup) => {
            // Handle tuple unpacking: (a, x) = ...
            for elt in &tup.elts {
                check_final_assign_target(elt, global_finals, local_finals, out);
            }
        }
        _ => {}
    }
}

/// Check an expression for walrus operator assignments to Final variables.
pub(super) fn check_walrus_final(
    expr: &Expr,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &std::collections::HashSet<String>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    if let Expr::Named(named) = expr {
        let Expr::Name(n) = named.target.as_ref() else {
            return;
        };
        let name = n.id.as_str();
        if global_finals.contains(name) {
            out.push(FinalViolationInfo {
                kind: FinalViolationKind::GlobalFinalModification,
                span: text_range_to_span(n.range()),
                name: name.to_string(),
            });
        } else if local_finals.contains(name) {
            out.push(FinalViolationInfo {
                kind: FinalViolationKind::FunctionLocalFinalModification,
                span: text_range_to_span(n.range()),
                name: name.to_string(),
            });
        }
    }
}
