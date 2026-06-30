//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Class Info visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef, StmtIf};
use ruff_text_size::Ranged;

use crate::scope::{AttributeInfo, FunctionInfo, MatchStmtInfo, RhsKind};
use crate::static_condition::{parse_static_condition, StaticCondition};

/// Return type for [`collect_class_body`]: attributes, method names, and
/// per-method decorator lists.
pub(super) type ClassBodyInfo = (Vec<AttributeInfo>, Vec<String>, Vec<(String, Vec<String>)>);

use super::annotations::{
    annotation_contains_readonly_expr, annotation_is_init_var, annotation_is_kw_only,
};
use super::class_info_ext::{class_info_from, expr_simple_name};
use super::core::{classify_rhs, collect_from_body, text_range_to_span};
use super::dataclass::{field_init_is_false, field_kw_only_override};
use super::function_info::function_info_from;

/// Flag the closures collected from a method body (those with no `class_name`)
/// as lexically nested inside the class, starting at index `from`.  This keeps
/// their `Self` usage valid for `generics_self_usage` instead of being treated as
/// module-level.
fn mark_nested_in_class(functions: &mut [FunctionInfo], from: usize) {
    if let Some(nested) = functions.get_mut(from..) {
        for func in nested {
            if func.class_name.is_none() {
                func.nested_in_class = true;
            }
        }
    }
}

pub(super) fn collect_class_body(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    class_kw_only: bool,
) -> ClassBodyInfo {
    let mut attributes = Vec::new();
    let mut method_names = Vec::new();
    let mut method_decorators: Vec<(String, Vec<String>)> = Vec::new();
    // Track whether we have passed the `_: KW_ONLY` sentinel.
    let mut after_kw_only_sentinel = false;

    for stmt in &class.body {
        match stmt {
            Stmt::AnnAssign(ann) => {
                // Detect `_: KW_ONLY` sentinel — skip it as a real attribute.
                if is_kw_only_sentinel(ann) {
                    after_kw_only_sentinel = true;
                } else if let Some(attr) =
                    ann_attribute(ann, after_kw_only_sentinel, class_kw_only, None)
                {
                    attributes.push(attr);
                }
            }
            Stmt::Assign(assign) => {
                assign_attributes(assign, None, &mut attributes);
            }
            // A `if sys.version_info >= (...)` / `if TYPE_CHECKING` guard inside a
            // class body conditionally defines fields. Collect them, tagged with
            // the guard so `resolve_with_target` can prune the ones that do not
            // exist for the target version.
            Stmt::If(if_stmt) => {
                collect_guarded_fields(
                    if_stmt,
                    None,
                    after_kw_only_sentinel,
                    class_kw_only,
                    &mut attributes,
                );
            }
            Stmt::FunctionDef(func) => {
                let func_info = function_info_from(func, Some(class.name.to_string()));
                let method_name = func_info.name.clone();
                let decs = func_info.decorators.clone();
                functions.push(func_info);
                // Any function collected from this method body is a closure
                // lexically nested inside the class; mark the non-method ones so
                // `generics_self_usage` does not treat their `Self` as module-level usage.
                let nested_start = functions.len();
                collect_from_body(
                    &func.body,
                    functions,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    match_stmts,
                    false,
                );
                mark_nested_in_class(functions, nested_start);
                method_names.push(method_name.clone());
                method_decorators.push((method_name, decs));
            }
            Stmt::ClassDef(inner_class) => {
                // Recurse into nested classes so their methods are checked
                // by BSK-E0001/BSK-E0002.  The inner ClassInfo is not added to the
                // module's class list (Phase 1 limitation), but all its
                // method FunctionInfos land in `functions`.
                let _inner_info = class_info_from(inner_class, functions, match_stmts);
            }
            _ => {}
        }
    }

    (attributes, method_names, method_decorators)
}

/// `true` for the `_: KW_ONLY` dataclass sentinel.
fn is_kw_only_sentinel(ann: &StmtAnnAssign) -> bool {
    matches!(expr_simple_name(&ann.target), Some(name) if name == "_")
        && annotation_is_kw_only(&ann.annotation)
}

/// Build an [`AttributeInfo`] from an annotated field (`name: T = value`), or
/// `None` when the target is not a simple name.
fn ann_attribute(
    ann: &StmtAnnAssign,
    after_kw_only_sentinel: bool,
    class_kw_only: bool,
    guard: Option<StaticCondition>,
) -> Option<AttributeInfo> {
    let name = expr_simple_name(&ann.target)?;
    let field_kw_only = ann.value.as_deref().and_then(field_kw_only_override);
    // Determine kw_only: explicit field() override wins; then sentinel; then class default.
    let is_kw_only = field_kw_only.unwrap_or(after_kw_only_sentinel || class_kw_only);
    Some(AttributeInfo {
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
        is_readonly: annotation_contains_readonly_expr(&ann.annotation),
        is_kw_only,
        is_init_false: ann.value.as_deref().is_some_and(field_init_is_false),
        is_init_var: annotation_is_init_var(&ann.annotation),
        guard,
    })
}

/// Append an [`AttributeInfo`] for each simple-name target of `name = value`.
fn assign_attributes(
    assign: &StmtAssign,
    guard: Option<&StaticCondition>,
    attributes: &mut Vec<AttributeInfo>,
) {
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
    for target in &assign.targets {
        if let Some(name) = expr_simple_name(target) {
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
                guard: guard.cloned(),
            });
        }
    }
}

/// Collect fields defined inside an `if`/`elif`/`else` chain in a class body,
/// tagging each with the static guard of its branch (AND-combined with any
/// enclosing guard). Methods and nested classes inside guards are intentionally
/// left to the existing unconditional handling.
fn collect_guarded_fields(
    if_stmt: &StmtIf,
    outer: Option<&StaticCondition>,
    after_kw_only_sentinel: bool,
    class_kw_only: bool,
    attributes: &mut Vec<AttributeInfo>,
) {
    let test = parse_static_condition(&if_stmt.test);
    let if_guard = combine_guards(outer, test.clone());
    collect_fields_in_branch(
        &if_stmt.body,
        &if_guard,
        after_kw_only_sentinel,
        class_kw_only,
        attributes,
    );

    // Each elif/else branch is reached only when every preceding test was false.
    let mut prior_negations = vec![StaticCondition::Not(Box::new(test))];
    for clause in &if_stmt.elif_else_clauses {
        let branch = match &clause.test {
            Some(elif_test) => {
                let cond = parse_static_condition(elif_test);
                let mut parts = prior_negations.clone();
                parts.push(cond.clone());
                prior_negations.push(StaticCondition::Not(Box::new(cond)));
                StaticCondition::All(parts)
            }
            None => StaticCondition::All(prior_negations.clone()),
        };
        let guard = combine_guards(outer, branch);
        collect_fields_in_branch(
            &clause.body,
            &guard,
            after_kw_only_sentinel,
            class_kw_only,
            attributes,
        );
    }
}

/// Collect the field statements (and nested `if`s) of a single guarded branch.
fn collect_fields_in_branch(
    stmts: &[Stmt],
    guard: &StaticCondition,
    after_kw_only_sentinel: bool,
    class_kw_only: bool,
    attributes: &mut Vec<AttributeInfo>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) if !is_kw_only_sentinel(ann) => {
                if let Some(attr) = ann_attribute(
                    ann,
                    after_kw_only_sentinel,
                    class_kw_only,
                    Some(guard.clone()),
                ) {
                    attributes.push(attr);
                }
            }
            Stmt::Assign(assign) => {
                assign_attributes(assign, Some(guard), attributes);
            }
            Stmt::If(nested) => {
                collect_guarded_fields(
                    nested,
                    Some(guard),
                    after_kw_only_sentinel,
                    class_kw_only,
                    attributes,
                );
            }
            _ => {}
        }
    }
}

/// AND-combine an optional enclosing guard with an inner one.
fn combine_guards(outer: Option<&StaticCondition>, inner: StaticCondition) -> StaticCondition {
    match outer {
        Some(existing) => StaticCondition::All(vec![existing.clone(), inner]),
        None => inner,
    }
}
