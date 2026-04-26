//! Helper types and AST collection functions for BSK-E0138.
//!
//! Contains data types describing `@dataclass_transform` metaclasses and
//! derived classes, plus the AST-scanning passes that populate them.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0138",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0138",
};

// ---------------------------------------------------------------------------
// Transform descriptor — one per metaclass
// ---------------------------------------------------------------------------

/// Describes the options declared on `@dataclass_transform(...)`.
#[derive(Debug, Clone)]
pub(super) struct TransformDesc {
    /// `kw_only_default` from `@dataclass_transform(kw_only_default=True)`.
    pub(super) kw_only_default: bool,
    /// `frozen_default` from `@dataclass_transform(frozen_default=True)`.
    pub(super) frozen_default: bool,
}

// ---------------------------------------------------------------------------
// Class descriptor — one per class that uses a transform metaclass
// ---------------------------------------------------------------------------

/// Describes a class that inherits from a `@dataclass_transform` base.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors dataclass_transform keyword flags"
)]
pub(super) struct TransformClassDesc {
    pub(super) name: String,
    /// `frozen=True/False` from the class keyword args (overrides `frozen_default`).
    pub(super) frozen: bool,
    /// `order=True/False` from the class keyword args.
    pub(super) order: bool,
    /// `kw_only=True` from the class keyword args (overrides `kw_only_default`).
    pub(super) kw_only: bool,
    /// `kw_only_default` resolved for this class.
    pub(super) kw_only_effective: bool,
    /// Span of the class `def` statement for reporting.
    pub(super) def_span: Span,
}

// ---------------------------------------------------------------------------
// AST collection helpers
// ---------------------------------------------------------------------------

/// Collect metaclass names decorated with `@dataclass_transform(...)`.
pub(super) fn collect_transform_metaclasses(stmts: &[Stmt]) -> HashMap<String, TransformDesc> {
    let mut out = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        for dec in &cls.decorator_list {
            let (is_dt, kw_only_default, frozen_default) =
                parse_dataclass_transform_expr(&dec.expression);
            if is_dt {
                let _ = out.insert(
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
pub(super) fn parse_dataclass_transform_expr(expr: &Expr) -> (bool, bool, bool) {
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
pub(super) fn collect_transform_bases(
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
                let _ = out.insert(cls.name.to_string(), meta_name.id.to_string());
            }
        }
    }
    out
}

/// Collect classes that inherit from transform bases (direct and transitive).
pub(super) fn collect_transform_classes(
    stmts: &[Stmt],
    transform_bases: &HashMap<String, String>,
    meta_classes: &HashMap<String, TransformDesc>,
) -> Vec<TransformClassDesc> {
    let mut out = collect_direct_transform_classes(stmts, transform_bases, meta_classes);
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
                    return transform_bases
                        .get(base_name)
                        .map(std::string::String::as_str);
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
pub(super) fn build_class_desc_from_meta(
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
pub(super) fn class_keyword_bool(cls: &ruff_python_ast::StmtClassDef, key: &str) -> Option<bool> {
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
pub(super) fn build_instance_class_map(
    stmts: &[Stmt],
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
                let _ = out.insert(var_name.id.to_string(), callee.to_string());
            }
        }
    }
    out
}

/// Check a single expression for a call to a kw-only class with positional args.
pub(super) fn check_call_expr(
    expr: &Expr,
    kw_only_classes: &std::collections::HashSet<&str>,
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
        provenance: None,
    });
}
