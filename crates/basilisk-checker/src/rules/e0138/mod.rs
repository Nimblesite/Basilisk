//! BSK-E0138: `dataclass_transform` metaclass violations.
//!
//! Detects type errors in classes whose metaclass is decorated with
//! `@dataclass_transform(...)`. Four violation kinds are covered:
//!
//! 1. **Frozen inheritance**: a non-frozen subclass inheriting from a frozen one.
//! 2. **Frozen attribute assignment**: mutating an attribute of a frozen instance.
//! 3. **Positional argument to kw-only constructor**: all fields are keyword-only
//!    when `kw_only_default=True` on the transform.
//! 4. **Ordering comparison without `order`**: using `<`/`<=`/`>`/`>=` on instances
//!    of a class that did not opt in to `order=True`.
//!
//! ```python
//! from typing import dataclass_transform
//!
//! @dataclass_transform(kw_only_default=True)
//! class ModelMeta(type): ...
//!
//! class ModelBase(metaclass=ModelMeta): ...
//!
//! class Customer(ModelBase, frozen=True):
//!     id: int
//!
//! c = Customer(id=1)
//! c.id = 2        # E — frozen
//! v = c < c       # E — no ordering methods
//! ```

mod helpers;

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::Diagnostic;

use super::Rule;

use helpers::{
    build_instance_class_map, check_call_expr, class_keyword_bool, collect_transform_bases,
    collect_transform_classes, collect_transform_metaclasses, TransformClassDesc, CODE,
};

/// Emits BSK-E0138 for violations related to `@dataclass_transform` metaclasses.
pub(crate) struct DataclassTransformMetaViolation;

impl Rule for DataclassTransformMetaViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        let ctx = MetaTransformCtx::from_ast(&parsed.ast.body);
        if ctx.meta_classes.is_empty() {
            return;
        }

        ctx.check_all(&parsed.ast.body, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Context built from a single module AST
// ---------------------------------------------------------------------------

struct MetaTransformCtx {
    /// map from metaclass name -> transform descriptor.
    meta_classes: HashMap<String, helpers::TransformDesc>,
    /// map from "base class that has a dataclass-transform metaclass" -> metaclass name.
    #[expect(dead_code)]
    transform_bases: HashMap<String, String>,
    /// all classes that inherit from a transform base.
    transform_classes: Vec<TransformClassDesc>,
}

impl MetaTransformCtx {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let meta_classes = collect_transform_metaclasses(stmts);
        let transform_bases = collect_transform_bases(stmts, &meta_classes);
        let transform_classes = collect_transform_classes(stmts, &transform_bases, &meta_classes);

        Self {
            meta_classes,
            transform_bases,
            transform_classes,
        }
    }

    fn check_all(&self, stmts: &[Stmt], path: &str, diag: &mut Vec<Diagnostic>) {
        let class_map: HashMap<&str, &TransformClassDesc> = self
            .transform_classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        self.check_frozen_inheritance(stmts, &class_map, path, diag);

        let frozen_classes: HashSet<&str> = class_map
            .values()
            .filter(|c| c.frozen)
            .map(|c| c.name.as_str())
            .collect();

        let order_classes: HashSet<&str> = class_map
            .values()
            .filter(|c| c.order)
            .map(|c| c.name.as_str())
            .collect();

        let instance_class = build_instance_class_map(stmts, &class_map);

        self.check_frozen_assign(stmts, &frozen_classes, &instance_class, path, diag);
        self.check_kw_only_calls(stmts, &class_map, path, diag);
        self.check_order_comparisons(stmts, &order_classes, &instance_class, path, diag);
    }

    // -----------------------------------------------------------------------
    // Frozen inheritance
    // -----------------------------------------------------------------------

    #[expect(
        clippy::unused_self,
        reason = "method on MetaTransformCtx for consistency"
    )]
    fn check_frozen_inheritance(
        &self,
        stmts: &[Stmt],
        class_map: &HashMap<&str, &TransformClassDesc>,
        path: &str,
        diag: &mut Vec<Diagnostic>,
    ) {
        for stmt in stmts {
            let Stmt::ClassDef(cls) = stmt else {
                continue;
            };
            let Some(desc) = class_map.get(cls.name.as_str()) else {
                continue;
            };

            let explicit_non_frozen = class_keyword_bool(cls, "frozen") == Some(false);
            if !explicit_non_frozen {
                continue;
            }

            let Some(args) = &cls.arguments else {
                continue;
            };
            for base_expr in &args.args {
                let Expr::Name(base_name) = base_expr else {
                    continue;
                };
                let base = base_name.id.as_str();
                let Some(base_desc) = class_map.get(base) else {
                    continue;
                };
                if base_desc.frozen {
                    diag.push(crate::diagnostic::Diagnostic {
                        code: CODE.clone(),
                        severity: crate::diagnostic::Severity::Error,
                        message: format!(
                            "Non-frozen class `{}` cannot inherit from frozen \
                             dataclass_transform class `{}`",
                            desc.name, base
                        ),
                        span: desc.def_span,
                        path: path.to_owned(),
                        help: Some(
                            "A non-frozen class cannot inherit from a frozen one when \
                             the metaclass uses `@dataclass_transform`"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 681: mixing frozen and non-frozen \
                             dataclass_transform classes is not allowed"
                                .to_owned(),
                        ),
                    });
                    break;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Frozen attribute assignment
    // -----------------------------------------------------------------------

    #[expect(
        clippy::unused_self,
        reason = "method on MetaTransformCtx for consistency"
    )]
    fn check_frozen_assign(
        &self,
        stmts: &[Stmt],
        frozen_classes: &HashSet<&str>,
        instance_class: &HashMap<String, String>,
        path: &str,
        diag: &mut Vec<Diagnostic>,
    ) {
        if frozen_classes.is_empty() || instance_class.is_empty() {
            return;
        }

        for stmt in stmts {
            let Stmt::Assign(assign) = stmt else {
                continue;
            };
            for target in &assign.targets {
                let Expr::Attribute(attr_expr) = target else {
                    continue;
                };
                let Expr::Name(obj) = attr_expr.value.as_ref() else {
                    continue;
                };
                let obj_name = obj.id.as_str();
                let Some(class_name) = instance_class.get(obj_name) else {
                    continue;
                };
                if !frozen_classes.contains(class_name.as_str()) {
                    continue;
                }
                let span = Span {
                    start: target.range().start().to_u32(),
                    end: target.range().end().to_u32(),
                };
                diag.push(crate::diagnostic::Diagnostic {
                    code: CODE.clone(),
                    severity: crate::diagnostic::Severity::Error,
                    message: format!(
                        "Cannot assign to attribute `{}` of frozen \
                         dataclass_transform class `{}` instance `{}`",
                        attr_expr.attr, class_name, obj_name
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(
                        "Frozen dataclass_transform instances are immutable after \
                         construction"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 681: `frozen=True` (or `frozen_default=True`) prohibits \
                         attribute assignment"
                            .to_owned(),
                    ),
                });
            }
        }
    }

    // -----------------------------------------------------------------------
    // kw-only positional argument violations
    // -----------------------------------------------------------------------

    #[expect(
        clippy::unused_self,
        reason = "method on MetaTransformCtx for consistency"
    )]
    fn check_kw_only_calls(
        &self,
        stmts: &[Stmt],
        class_map: &HashMap<&str, &TransformClassDesc>,
        path: &str,
        diag: &mut Vec<Diagnostic>,
    ) {
        let kw_only_classes: HashSet<&str> = class_map
            .values()
            .filter(|c| c.kw_only_effective)
            .map(|c| c.name.as_str())
            .collect();

        if kw_only_classes.is_empty() {
            return;
        }

        for stmt in stmts {
            match stmt {
                Stmt::Assign(assign) => {
                    check_call_expr(&assign.value, &kw_only_classes, path, diag);
                }
                Stmt::AnnAssign(ann) => {
                    if let Some(val) = &ann.value {
                        check_call_expr(val, &kw_only_classes, path, diag);
                    }
                }
                Stmt::Expr(expr_stmt) => {
                    check_call_expr(&expr_stmt.value, &kw_only_classes, path, diag);
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Ordering comparison without order=True
    // -----------------------------------------------------------------------

    fn check_order_comparisons(
        &self,
        stmts: &[Stmt],
        order_classes: &HashSet<&str>,
        instance_class: &HashMap<String, String>,
        path: &str,
        diag: &mut Vec<Diagnostic>,
    ) {
        let transform_class_names: HashSet<&str> = self
            .transform_classes
            .iter()
            .map(|c| c.name.as_str())
            .collect();

        if transform_class_names.is_empty() || instance_class.is_empty() {
            return;
        }

        for stmt in stmts {
            let Stmt::Assign(assign) = stmt else {
                continue;
            };
            let Expr::Compare(cmp) = assign.value.as_ref() else {
                continue;
            };
            let has_ordering = cmp.ops.iter().any(|op| {
                matches!(
                    op,
                    ruff_python_ast::CmpOp::Lt
                        | ruff_python_ast::CmpOp::LtE
                        | ruff_python_ast::CmpOp::Gt
                        | ruff_python_ast::CmpOp::GtE
                )
            });
            if !has_ordering {
                continue;
            }

            let Expr::Name(left_name) = cmp.left.as_ref() else {
                continue;
            };
            let left = left_name.id.as_str();
            let Some(left_class) = instance_class.get(left) else {
                continue;
            };

            if !transform_class_names.contains(left_class.as_str()) {
                continue;
            }
            if order_classes.contains(left_class.as_str()) {
                continue;
            }

            let span = Span {
                start: cmp.range().start().to_u32(),
                end: cmp.range().end().to_u32(),
            };
            diag.push(crate::diagnostic::Diagnostic {
                code: CODE.clone(),
                severity: crate::diagnostic::Severity::Error,
                message: format!(
                    "Ordering comparison on `{left}` instance: class `{left_class}` does not \
                     synthesize comparison methods (missing `order=True`)",
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass `order=True` to `{left_class}(...)` to enable ordering comparisons"
                )),
                note: Some(
                    "PEP 681: ordering methods are only synthesized when `order=True` \
                     is passed to the dataclass_transform class"
                        .to_owned(),
                ),
            });
        }
    }
}
