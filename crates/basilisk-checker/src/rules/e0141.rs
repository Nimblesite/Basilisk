//! BSK-E0141: Unpack[`TypedDict`] kwargs violations.
//!
//! Detects invalid uses of `**kwargs: Unpack[TypedDict]` in function signatures:
//! parameter overlap with `TypedDict` keys, and `Unpack[TypeVar]` (invalid).

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0141",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0141",
};

/// Emits BSK-E0141 for Unpack[`TypedDict`] kwargs violations.
pub(crate) struct UnpackKwargsViolation;

impl Rule for UnpackKwargsViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        let ctx = KwargsContext::from_ast(&parsed.ast.body);
        for stmt in &parsed.ast.body {
            if let Stmt::FunctionDef(func) = stmt {
                check_function(func, &ctx, &module.path, diagnostics);
            }
        }
    }
}

struct KwargsContext {
    typeddict_keys: Vec<(String, Vec<String>)>,
    typevar_names: Vec<String>,
}

impl KwargsContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut typeddict_keys: Vec<(String, Vec<String>)> = Vec::new();
        let mut typevar_names = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    if is_typeddict(cls) {
                        let keys: Vec<String> = cls
                            .body
                            .iter()
                            .filter_map(|s| {
                                if let Stmt::AnnAssign(ann) = s {
                                    expr_name(&ann.target).map(std::borrow::ToOwned::to_owned)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        // Also collect keys from base TypedDicts
                        let mut all_keys = Vec::new();
                        if let Some(args) = &cls.arguments {
                            for base in &args.args {
                                if let Expr::Name(n) = base {
                                    let base_name = n.id.as_str();
                                    if let Some((_, bkeys)) =
                                        typeddict_keys.iter().find(|(n, _)| n == base_name)
                                    {
                                        all_keys.extend(bkeys.iter().cloned());
                                    }
                                }
                            }
                        }
                        all_keys.extend(keys);
                        typeddict_keys.push((cls.name.to_string(), all_keys));
                    }
                }
                Stmt::Assign(assign) => {
                    if assign.targets.len() == 1 {
                        if let Some(name) = expr_name(&assign.targets[0]) {
                            if is_typevar_call(&assign.value) {
                                typevar_names.push(name.to_owned());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Self {
            typeddict_keys,
            typevar_names,
        }
    }
    fn get_td_keys(&self, name: &str) -> Option<&[String]> {
        self.typeddict_keys
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| k.as_slice())
    }
    fn is_typevar(&self, name: &str) -> bool {
        self.typevar_names.iter().any(|n| n == name)
    }
}

fn is_typeddict(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args
            .iter()
            .any(|a| matches!(a, Expr::Name(n) if n.id.as_str() == "TypedDict"))
    })
}

fn is_typevar_call(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(call) if matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "TypeVar"))
}

fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

fn check_function(
    func: &ast::StmtFunctionDef,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(kwarg) = &func.parameters.kwarg else {
        return;
    };
    let Some(annotation) = &kwarg.annotation else {
        return;
    };
    let Some(unpack_type) = extract_unpack_arg(annotation) else {
        return;
    };

    let func_span = Span {
        start: func.range().start().to_u32(),
        end: func.range().end().to_u32(),
    };

    // Check: Unpack[TypeVar] is invalid
    if ctx.is_typevar(unpack_type) {
        diag.push(Diagnostic {
            code: CODE.clone(), severity: Severity::Error,
            message: format!("Invalid `**kwargs: Unpack[{unpack_type}]`: `{unpack_type}` is a TypeVar, not a TypedDict"),
            span: func_span, path: path.to_owned(),
            help: Some("Use a concrete TypedDict type with Unpack".to_owned()), note: None,
        });
        return;
    }

    // Check: parameter overlap with TypedDict keys
    let Some(td_keys) = ctx.get_td_keys(unpack_type) else {
        return;
    };
    let non_posonly: Vec<&str> = func
        .parameters
        .args
        .iter()
        .map(|p| p.parameter.name.as_str())
        .collect();
    for pname in &non_posonly {
        if td_keys.iter().any(|k| k == pname) {
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Parameter `{pname}` overlaps with TypedDict `{unpack_type}` key `{pname}`"
                ),
                span: func_span,
                path: path.to_owned(),
                help: Some(format!("Make `{pname}` positional-only (add `/`)")),
                note: None,
            });
        }
    }
}

fn extract_unpack_arg(expr: &Expr) -> Option<&str> {
    if let Expr::Subscript(sub) = expr {
        if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Unpack") {
            return expr_name(&sub.slice);
        }
    }
    None
}
