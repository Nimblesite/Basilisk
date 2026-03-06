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

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0138",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0138",
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
// Transform descriptor — one per metaclass
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TransformDesc {
    /// `kw_only_default` from `@dataclass_transform(kw_only_default=True)`.
    kw_only_default: bool,
    /// `frozen_default` from `@dataclass_transform(frozen_default=True)`.
    frozen_default: bool,
}

// ---------------------------------------------------------------------------
// Class descriptor — one per class that uses a transform metaclass
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
struct TransformClassDesc {
    name: String,
    /// `frozen=True/False` from the class keyword args (overrides `frozen_default`).
    frozen: bool,
    /// `order=True/False` from the class keyword args.
    order: bool,
    /// `kw_only=True` from the class keyword args (overrides `kw_only_default`).
    kw_only: bool,
    /// `kw_only_default` resolved for this class.
    kw_only_effective: bool,
    /// Line of the class `def` for span reporting.
    def_span: Span,
}

// ---------------------------------------------------------------------------
// Context built from a single module AST
// ---------------------------------------------------------------------------

struct MetaTransformCtx {
    /// map from metaclass name -> transform descriptor.
    meta_classes: HashMap<String, TransformDesc>,
    /// map from "base class that has a dataclass-transform metaclass" -> metaclass name.
    #[allow(dead_code)]
    transform_bases: HashMap<String, String>,
    /// all classes that inherit from a transform base.
    transform_classes: Vec<TransformClassDesc>,
}

impl MetaTransformCtx {
    fn from_ast(stmts: &[Stmt]) -> Self {
        // Pass 1: collect metaclasses decorated with @dataclass_transform.
        let meta_classes = collect_transform_metaclasses(stmts);

        // Pass 2: collect classes that directly use one of those metaclasses.
        let transform_bases = collect_transform_bases(stmts, &meta_classes);

        // Pass 3: collect all subclasses of the transform bases (recursive 1-level).
        let transform_classes = collect_transform_classes(stmts, &transform_bases, &meta_classes);

        Self {
            meta_classes,
            transform_bases,
            transform_classes,
        }
    }

    fn check_all(&self, stmts: &[Stmt], path: &str, diag: &mut Vec<Diagnostic>) {
        // Build lookup maps.
        let class_map: HashMap<&str, &TransformClassDesc> = self
            .transform_classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        // Check 1: frozen inheritance.
        self.check_frozen_inheritance(stmts, &class_map, path, diag);

        // Identify frozen classes.
        let frozen_classes: HashSet<&str> = class_map
            .values()
            .filter(|c| c.frozen)
            .map(|c| c.name.as_str())
            .collect();

        // Identify order classes.
        let order_classes: HashSet<&str> = class_map
            .values()
            .filter(|c| c.order)
            .map(|c| c.name.as_str())
            .collect();

        // Build instance → class map from module-level assignments.
        let instance_class = build_instance_class_map(stmts, &frozen_classes, &class_map);

        // Check 2: frozen attribute assignment.
        self.check_frozen_assign(stmts, &frozen_classes, &instance_class, path, diag);

        // Check 3: kw-only positional argument violations.
        self.check_kw_only_calls(stmts, &class_map, path, diag);

        // Check 4: ordering comparison without order=True.
        self.check_order_comparisons(stmts, &order_classes, &instance_class, path, diag);
    }

    // -----------------------------------------------------------------------
    // Frozen inheritance
    // -----------------------------------------------------------------------

    #[allow(clippy::unused_self)]
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

            // Find keyword `frozen=False` explicitly.
            let explicit_non_frozen = class_keyword_bool(cls, "frozen") == Some(false);
            if !explicit_non_frozen {
                continue;
            }

            // Check whether any base class is frozen.
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
                    diag.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
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

    #[allow(clippy::unused_self)]
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
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
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

    #[allow(clippy::unused_self)]
    fn check_kw_only_calls(
        &self,
        stmts: &[Stmt],
        class_map: &HashMap<&str, &TransformClassDesc>,
        path: &str,
        diag: &mut Vec<Diagnostic>,
    ) {
        // Collect kw_only classes.
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
        // All transform classes.
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
            // We only care about ordering operators.
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

            // Check whether left class is a transform class without order=True.
            if !transform_class_names.contains(left_class.as_str()) {
                continue;
            }
            if order_classes.contains(left_class.as_str()) {
                // order=True is fine — ordering comparison is valid.
                continue;
            }

            let span = Span {
                start: cmp.range().start().to_u32(),
                end: cmp.range().end().to_u32(),
            };
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
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

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

/// Collect metaclass names decorated with `@dataclass_transform(...)`.
fn collect_transform_metaclasses(stmts: &[Stmt]) -> HashMap<String, TransformDesc> {
    let mut out = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        for dec in &cls.decorator_list {
            let (is_dt, kw_only_default, frozen_default) =
                parse_dataclass_transform_expr(&dec.expression);
            if is_dt {
                out.insert(
                    cls.name.to_string(),
                    TransformDesc {
                        kw_only_default,
                        frozen_default,
                    },
                );
                break;
            }
        }
    }
    out
}

/// Parse a `@dataclass_transform(...)` decorator expression.
///
/// Returns `(is_dataclass_transform, kw_only_default, frozen_default)`.
fn parse_dataclass_transform_expr(expr: &Expr) -> (bool, bool, bool) {
    // Bare `@dataclass_transform`
    if let Expr::Name(n) = expr {
        if n.id.as_str() == "dataclass_transform" {
            return (true, false, false);
        }
    }

    let Expr::Call(call) = expr else {
        return (false, false, false);
    };

    let is_dt = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "dataclass_transform",
        Expr::Attribute(a) => a.attr.as_str() == "dataclass_transform",
        _ => false,
    };
    if !is_dt {
        return (false, false, false);
    }

    let mut kw_only_default = false;
    let mut frozen_default = false;

    for kw in &call.arguments.keywords {
        let Some(arg_name) = kw.arg.as_ref() else {
            continue;
        };
        match arg_name.as_str() {
            "kw_only_default" => {
                kw_only_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "frozen_default" => {
                frozen_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            _ => {}
        }
    }

    (true, kw_only_default, frozen_default)
}

/// Collect classes that directly specify `metaclass=<transform_meta>`.
///
/// Returns a map from the base class name to the metaclass name it uses.
fn collect_transform_bases(
    stmts: &[Stmt],
    meta_classes: &HashMap<String, TransformDesc>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let Some(args) = &cls.arguments else {
            continue;
        };
        for kw in &args.keywords {
            let Some(kw_name) = kw.arg.as_ref() else {
                continue;
            };
            if kw_name.as_str() != "metaclass" {
                continue;
            }
            let Expr::Name(meta_name) = &kw.value else {
                continue;
            };
            if meta_classes.contains_key(meta_name.id.as_str()) {
                out.insert(cls.name.to_string(), meta_name.id.to_string());
            }
        }
    }
    out
}

/// Collect classes that inherit from transform bases, and also classes that
/// inherit from those transform classes (transitive). This handles the case
/// where e.g. `Customer1Subclass(Customer1)` inherits from a transform class
/// rather than directly from a transform base.
fn collect_transform_classes(
    stmts: &[Stmt],
    transform_bases: &HashMap<String, String>,
    meta_classes: &HashMap<String, TransformDesc>,
) -> Vec<TransformClassDesc> {
    // Pass 1: collect classes that directly inherit from a transform base.
    let mut out = collect_direct_transform_classes(stmts, transform_bases, meta_classes);

    // Pass 2: collect classes that inherit from already-collected transform classes.
    collect_inherited_transform_classes(stmts, transform_bases, meta_classes, &mut out);

    out
}

/// Collect classes that directly inherit from a transform base.
fn collect_direct_transform_classes(
    stmts: &[Stmt],
    transform_bases: &HashMap<String, String>,
    meta_classes: &HashMap<String, TransformDesc>,
) -> Vec<TransformClassDesc> {
    let mut out = Vec::new();

    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let Some(args) = &cls.arguments else {
            continue;
        };

        let matched_meta: Option<&str> = args.args.iter().find_map(|base| {
            if let Expr::Name(n) = base {
                let base_name = n.id.as_str();
                if transform_bases.contains_key(base_name) {
                    return transform_bases.get(base_name).map(std::string::String::as_str);
                }
            }
            None
        });

        let Some(meta_name) = matched_meta else {
            continue;
        };
        let Some(desc) = meta_classes.get(meta_name) else {
            continue;
        };

        out.push(build_class_desc_from_meta(cls, desc));
    }

    out
}

/// Build a `TransformClassDesc` from a class definition and its metaclass descriptor.
fn build_class_desc_from_meta(
    cls: &ruff_python_ast::StmtClassDef,
    desc: &TransformDesc,
) -> TransformClassDesc {
    let frozen_kw = class_keyword_bool(cls, "frozen");
    let order_kw = class_keyword_bool(cls, "order");
    let kw_only_kw = class_keyword_bool(cls, "kw_only");

    let frozen = frozen_kw.unwrap_or(desc.frozen_default);
    let order = order_kw.unwrap_or(false);
    let kw_only = kw_only_kw.unwrap_or(false);
    let kw_only_effective = kw_only || desc.kw_only_default;

    let def_span = Span {
        start: cls.range().start().to_u32(),
        end: cls.range().end().to_u32(),
    };

    TransformClassDesc {
        name: cls.name.to_string(),
        frozen,
        order,
        kw_only,
        kw_only_effective,
        def_span,
    }
}

/// Collect classes that inherit from already-collected transform classes (transitive).
fn collect_inherited_transform_classes(
    stmts: &[Stmt],
    transform_bases: &HashMap<String, String>,
    meta_classes: &HashMap<String, TransformDesc>,
    out: &mut Vec<TransformClassDesc>,
) {
    let transform_class_names: HashMap<String, String> = out
        .iter()
        .filter_map(|tc| {
            stmts.iter().find_map(|stmt| {
                let Stmt::ClassDef(cls) = stmt else {
                    return None;
                };
                if cls.name.as_str() != tc.name {
                    return None;
                }
                let args = cls.arguments.as_ref()?;
                args.args.iter().find_map(|base| {
                    if let Expr::Name(n) = base {
                        transform_bases
                            .get(n.id.as_str())
                            .map(|meta| (tc.name.clone(), meta.clone()))
                    } else {
                        None
                    }
                })
            })
        })
        .collect();

    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let Some(args) = &cls.arguments else {
            continue;
        };

        if out.iter().any(|tc| tc.name == cls.name.as_str()) {
            continue;
        }

        let parent_tc = args.args.iter().find_map(|base| {
            if let Expr::Name(n) = base {
                let base_name = n.id.as_str();
                let parent = out.iter().find(|tc| tc.name == base_name)?;
                let meta_name = transform_class_names.get(base_name)?;
                Some((parent.clone(), meta_name.clone()))
            } else {
                None
            }
        });

        let Some((parent_desc, meta_name)) = parent_tc else {
            continue;
        };
        let Some(meta_desc) = meta_classes.get(&meta_name) else {
            continue;
        };

        let frozen_kw = class_keyword_bool(cls, "frozen");
        let order_kw = class_keyword_bool(cls, "order");
        let kw_only_kw = class_keyword_bool(cls, "kw_only");

        let frozen = frozen_kw.unwrap_or(parent_desc.frozen);
        let order = order_kw.unwrap_or(parent_desc.order);
        let kw_only = kw_only_kw.unwrap_or(parent_desc.kw_only);
        let kw_only_effective = kw_only || meta_desc.kw_only_default;

        let def_span = Span {
            start: cls.range().start().to_u32(),
            end: cls.range().end().to_u32(),
        };

        out.push(TransformClassDesc {
            name: cls.name.to_string(),
            frozen,
            order,
            kw_only,
            kw_only_effective,
            def_span,
        });
    }
}

/// Read a boolean keyword arg from a class definition's keyword arguments.
///
/// Returns `Some(true)`/`Some(false)` when the keyword is present,
/// `None` when absent.
fn class_keyword_bool(cls: &ruff_python_ast::StmtClassDef, key: &str) -> Option<bool> {
    let args = cls.arguments.as_ref()?;
    for kw in &args.keywords {
        let kw_name = kw.arg.as_ref()?;
        if kw_name.as_str() == key {
            return Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value));
        }
    }
    None
}

/// Build a map from variable name → class name for module-level assignments
/// where the RHS is a call to a transform class.
fn build_instance_class_map(
    stmts: &[Stmt],
    _frozen_classes: &HashSet<&str>,
    class_map: &HashMap<&str, &TransformClassDesc>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        let callee = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str(),
            Expr::Attribute(a) => a.attr.as_str(),
            _ => continue,
        };
        if !class_map.contains_key(callee) {
            continue;
        }
        for target in &assign.targets {
            if let Expr::Name(var_name) = target {
                out.insert(var_name.id.to_string(), callee.to_string());
            }
        }
    }
    out
}

/// Check a single expression for a call to a kw-only class with positional args.
fn check_call_expr(
    expr: &Expr,
    kw_only_classes: &HashSet<&str>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else {
        return;
    };
    let callee = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str(),
        Expr::Attribute(a) => a.attr.as_str(),
        _ => return,
    };
    if !kw_only_classes.contains(callee) {
        return;
    }
    if call.arguments.args.is_empty() {
        return;
    }
    let span = Span {
        start: call.range().start().to_u32(),
        end: call.range().end().to_u32(),
    };
    diag.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Positional argument(s) passed to `{callee}` whose constructor is \
             keyword-only (dataclass_transform `kw_only_default=True`)"
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "All arguments to `{callee}` must be passed as keyword arguments"
        )),
        note: Some(
            "PEP 681: when `kw_only_default=True` on the transform metaclass, \
             all fields are keyword-only unless explicitly overridden"
                .to_owned(),
        ),
    });
}
