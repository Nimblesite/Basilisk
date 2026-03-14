//! Class Info visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtClassDef};
use ruff_text_size::Ranged;

use crate::scope::{AttributeInfo, FunctionInfo, MatchStmtInfo, RhsKind};

use super::annotations::{
    annotation_contains_readonly_expr, annotation_is_init_var, annotation_is_kw_only,
};
use super::class_info_ext::{class_info_from, expr_simple_name};
use super::core::{classify_rhs, collect_from_body, text_range_to_span};
use super::dataclass::{field_init_is_false, field_kw_only_override};
use super::function_info::function_info_from;

pub(super) fn collect_class_body(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    class_kw_only: bool,
) -> (Vec<AttributeInfo>, Vec<String>, Vec<(String, Vec<String>)>) {
    let mut attributes = Vec::new();
    let mut method_names = Vec::new();
    let mut method_decorators: Vec<(String, Vec<String>)> = Vec::new();
    // Track whether we have passed the `_: KW_ONLY` sentinel.
    let mut after_kw_only_sentinel = false;

    for stmt in &class.body {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_simple_name(&ann.target) {
                    // Detect `_: KW_ONLY` sentinel — skip it as a real attribute.
                    if name == "_" && annotation_is_kw_only(&ann.annotation) {
                        after_kw_only_sentinel = true;
                        continue;
                    }
                    let is_readonly = annotation_contains_readonly_expr(&ann.annotation);
                    let is_init_var = annotation_is_init_var(&ann.annotation);
                    let field_kw_only = ann.value.as_deref().and_then(field_kw_only_override);
                    // Determine kw_only: explicit field() override wins; then sentinel; then class default.
                    let is_kw_only =
                        field_kw_only.unwrap_or(after_kw_only_sentinel || class_kw_only);
                    let is_init_false = ann.value.as_deref().is_some_and(field_init_is_false);
                    attributes.push(AttributeInfo {
                        name,
                        name_span: text_range_to_span(ann.target.range()),
                        has_annotation: true,
                        annotation_span: Some(text_range_to_span(ann.annotation.range())),
                        has_value: ann.value.is_some(),
                        rhs_kind: RhsKind::Other,
                        rhs_span: ann.value.as_ref().map(|v| text_range_to_span(v.range())),
                        rhs_is_nonmember_call: false,
                        rhs_is_lambda: false,
                        rhs_is_descriptor_call: false,
                        is_readonly,
                        is_kw_only,
                        is_init_false,
                        is_init_var,
                    });
                }
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Some(name) = expr_simple_name(target) {
                        let rhs_is_nonmember_call = matches!(
                            &*assign.value,
                            Expr::Call(c) if matches!(c.func.as_ref(), Expr::Name(n) if n.id == "nonmember")
                        );
                        let rhs_is_lambda = matches!(&*assign.value, Expr::Lambda(_));
                        let rhs_is_descriptor_call = matches!(
                            &*assign.value,
                            Expr::Call(c) if matches!(
                                c.func.as_ref(),
                                Expr::Name(n) if n.id == "staticmethod" || n.id == "classmethod"
                            )
                        );
                        attributes.push(AttributeInfo {
                            name,
                            name_span: text_range_to_span(target.range()),
                            has_annotation: false,
                            annotation_span: None,
                            has_value: true,
                            rhs_kind: classify_rhs(&assign.value),
                            rhs_span: Some(text_range_to_span(assign.value.range())),
                            rhs_is_nonmember_call,
                            rhs_is_lambda,
                            rhs_is_descriptor_call,
                            is_readonly: false,
                            is_kw_only: false,
                            is_init_false: false,
                            is_init_var: false,
                        });
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                let func_info = function_info_from(func, Some(class.name.to_string()));
                let method_name = func_info.name.clone();
                let decs = func_info.decorators.clone();
                functions.push(func_info);
                collect_from_body(
                    &func.body,
                    functions,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    match_stmts,
                    false,
                );
                method_names.push(method_name.clone());
                method_decorators.push((method_name, decs));
            }
            Stmt::ClassDef(inner_class) => {
                // Recurse into nested classes so their methods are checked
                // by E0001/E0002.  The inner ClassInfo is not added to the
                // module's class list (Phase 1 limitation), but all its
                // method FunctionInfos land in `functions`.
                let _inner_info = class_info_from(inner_class, functions, match_stmts);
            }
            _ => {}
        }
    }

    (attributes, method_names, method_decorators)
}
