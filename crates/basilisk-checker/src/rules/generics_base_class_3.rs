//! Implements [`generics_base_class_3`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `generics_base_class_3`: Invariant generic type mismatch at call site.
//!
//! When a function parameter expects a parameterised generic like
//! `dict[str, list[object]]` and a subclass whose base parameterisation
//! differs in an invariant position is passed, the call is invalid.
//!
//! ```python
//! class SymbolTable(dict[str, list[Node]]): ...
//!
//! def takes(x: dict[str, list[object]]): ...
//!
//! def test(s: SymbolTable):
//!     takes(s)  # E -- list is invariant, list[Node] != list[object]
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_base_class_3",
    docs_url: "https://www.basilisk-python.dev/errors/generics_base_class_3",
};

/// Emits `generics_base_class_3` for calls where a subclass argument is incompatible
/// with a parameterised generic parameter due to invariance.
pub(crate) struct InvariantGenericArgMismatch;

impl Rule for InvariantGenericArgMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Step 1: Build a map of class names to their resolved base type text.
        let class_base_map: HashMap<&str, (&str, Vec<&str>)> = build_class_base_map(module);

        if class_base_map.is_empty() {
            return;
        }

        // Step 2: Build a map of module-level function name -> params.
        let mut func_params: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            let mut params = Vec::new();
            for param in &func.parameters {
                let ann_text = param
                    .annotation_span
                    .and_then(|span| slice_span(source, span));
                if let Some(ann) = ann_text {
                    params.push((param.name.as_str(), ann.trim()));
                }
            }
            if !params.is_empty() {
                let _ = func_params.insert(func.name.as_str(), params);
            }
        }

        // Step 3: Re-parse and walk function bodies for calls.
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        for stmt in &parsed.ast.body {
            visit_stmt(
                stmt,
                source,
                path,
                &class_base_map,
                &func_params,
                diagnostics,
            );
        }
    }
}

/// Build a map from class name to (`base_generic_name`,`type_arg_texts`ts]).
fn build_class_base_map(module: &ResolvedModule) -> HashMap<&str, (&str, Vec<&str>)> {
    let source = &module.source;
    let mut map = HashMap::new();

    for cls in &module.classes {
        for entry in &cls.base_subscripts {
            if !is_builtin_generic(&entry.base_name) {
                continue;
            }
            let span_text = slice_span(source, entry.span);
            if let Some(text) = span_text {
                if let Some(type_args) = extract_subscript_args(text) {
                    let _ = map.insert(cls.name.as_str(), (entry.base_name.as_str(), type_args));
                }
            }
        }
    }

    map
}

/// Extract type argument texts from a subscript expression.
fn extract_subscript_args(text: &str) -> Option<Vec<&str>> {
    let bracket_pos = text.find('[')?;
    let inner = text.get(bracket_pos + 1..text.len().checked_sub(1)?)?;
    Some(split_top_level_commas(inner))
}

/// Returns `true` for builtin generic types.
fn is_builtin_generic(name: &str) -> bool {
    matches!(
        name,
        "dict" | "list" | "set" | "frozenset" | "Dict" | "List" | "Set" | "FrozenSet"
    )
}

/// Returns `true` for types that are invariant containers.
fn is_invariant_container(name: &str) -> bool {
    matches!(
        name,
        "list"
            | "List"
            | "dict"
            | "Dict"
            | "set"
            | "Set"
            | "frozenset"
            | "FrozenSet"
            | "deque"
            | "Deque"
    )
}

/// Walk statements to find function definitions and check bodies.
fn visit_stmt(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    class_base_map: &HashMap<&str, (&str, Vec<&str>)>,
    func_params: &HashMap<&str, Vec<(&str, &str)>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    if let Stmt::FunctionDef(func_def) = stmt {
        let func_param_types = build_param_type_map(func_def, source);

        for body_stmt in &func_def.body {
            check_body_stmt(
                body_stmt,
                source,
                path,
                class_base_map,
                func_params,
                &func_param_types,
                diagnostics,
            );
        }
    } else if let Stmt::ClassDef(cls) = stmt {
        for body_stmt in &cls.body {
            visit_stmt(
                body_stmt,
                source,
                path,
                class_base_map,
                func_params,
                diagnostics,
            );
        }
    }
}

/// Build a map from parameter name to its annotation text.
fn build_param_type_map(
    func_def: &ruff_python_ast::StmtFunctionDef,
    source: &str,
) -> HashMap<String, String> {
    use ruff_text_size::Ranged as _;

    let mut map = HashMap::new();
    for pwd in &func_def.parameters.args {
        let param = &pwd.parameter;
        if let Some(ann) = &param.annotation {
            let range = ann.range();
            if let Some(text) = source.get(range.start().to_usize()..range.end().to_usize()) {
                let _ = map.insert(param.name.to_string(), text.trim().to_string());
            }
        }
    }
    map
}

/// Check a statement inside a function body for calls with invariant
/// mismatches.
fn check_body_stmt(
    stmt: &ruff_python_ast::Stmt,
    _source: &str,
    path: &str,
    class_base_map: &HashMap<&str, (&str, Vec<&str>)>,
    func_params: &HashMap<&str, Vec<(&str, &str)>>,
    caller_params: &HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};
    use ruff_text_size::Ranged as _;

    let call = match stmt {
        Stmt::Expr(expr_stmt) => {
            if let Expr::Call(call) = expr_stmt.value.as_ref() {
                call
            } else {
                return;
            }
        }
        Stmt::Assign(assign) => {
            if let Expr::Call(call) = assign.value.as_ref() {
                call
            } else {
                return;
            }
        }
        _ => return,
    };

    let callee_name = match call.func.as_ref() {
        Expr::Name(name) => name.id.as_str(),
        _ => return,
    };

    let Some(callee_param_list) = func_params.get(callee_name) else {
        return;
    };

    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(&(param_name, param_ann)) = callee_param_list.get(arg_idx) else {
            continue;
        };

        let Expr::Name(arg_name) = arg_expr else {
            continue;
        };

        let Some(arg_type) = caller_params.get(arg_name.id.as_str()) else {
            continue;
        };

        let Some((class_base_generic, class_type_args)) = class_base_map.get(arg_type.as_str())
        else {
            continue;
        };

        let Some((param_generic, param_type_args)) = parse_generic_annotation(param_ann) else {
            continue;
        };

        if *class_base_generic != param_generic {
            continue;
        }

        for (class_arg, param_arg) in class_type_args.iter().zip(param_type_args.iter()) {
            if class_arg.trim() == param_arg.trim() {
                continue;
            }

            // Check nested invariant containers.
            let class_inner = parse_generic_annotation(class_arg);
            let param_inner = parse_generic_annotation(param_arg);

            if let (Some((cig, _)), Some((pig, _))) = (&class_inner, &param_inner) {
                if cig == pig && is_invariant_container(cig) {
                    emit_diagnostic(
                        callee_name,
                        param_name,
                        param_ann,
                        arg_type,
                        class_base_generic,
                        class_type_args,
                        class_arg,
                        param_arg,
                        cig,
                        call.range(),
                        path,
                        diagnostics,
                    );
                    return;
                }
            }

            // Direct invariant mismatch.
            if is_invariant_container(class_base_generic) {
                emit_diagnostic(
                    callee_name,
                    param_name,
                    param_ann,
                    arg_type,
                    class_base_generic,
                    class_type_args,
                    class_arg,
                    param_arg,
                    class_base_generic,
                    call.range(),
                    path,
                    diagnostics,
                );
                return;
            }
        }
    }
}

/// Emit the invariant generic argument mismatch diagnostic.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic formatting requires all context"
)]
fn emit_diagnostic(
    callee_name: &str,
    param_name: &str,
    param_ann: &str,
    arg_type: &str,
    class_base_generic: &str,
    class_type_args: &[&str],
    class_arg: &str,
    param_arg: &str,
    invariant_container: &str,
    range: ruff_text_size::TextRange,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let span = Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    };
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument `{param_name}` of `{callee_name}` expects \
             `{param_ann}` but received `{arg_type}` \
             (subclass of `{class_base_generic}[{}]`) -- \
             `{class_arg}` is not assignable to `{param_arg}` \
             because `{invariant_container}` is invariant",
            class_type_args.join(", ")
        ),
        span,
        path,
        Some(format!(
            "`{invariant_container}` is invariant: \
             `{class_arg}` is not a subtype of `{param_arg}`"
        )),
        Some(
            "Mutable generic containers like `list`, `dict`, `set` \
             are invariant in their type parameters."
                .to_owned(),
        ),
    ));
}

/// Parse a generic annotation like `dict[str, list[object]]`.
fn parse_generic_annotation(ann: &str) -> Option<(&str, Vec<&str>)> {
    let ann = ann.trim();
    let bracket_pos = ann.find('[')?;
    let name = ann.get(..bracket_pos)?.trim();
    let inner = ann.get(bracket_pos + 1..ann.len().checked_sub(1)?)?;
    Some((name, split_top_level_commas(inner)))
}
