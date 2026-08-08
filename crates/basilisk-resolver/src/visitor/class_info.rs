//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Class Info visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef, StmtIf};
use ruff_text_size::Ranged;

use crate::canonical::{BindingTable, TypingForm};
use crate::scope::{AttributeInfo, FunctionInfo, MatchStmtInfo, RhsKind};
use crate::static_condition::{parse_static_condition, StaticCondition};
use crate::visitor::class_info_ext::decorator_name;

/// Whether a class-body value is wrapped in the enum non-member marker, which
/// keeps it out of the enumeration's members.
///
/// Resolved through the module's bindings, so an aliased or module-qualified
/// use is recognised and a local definition of the same name is not.
fn rhs_is_nonmember_call(bindings: &BindingTable, value: &Expr) -> bool {
    matches!(value, Expr::Call(call) if bindings.is_form(&call.func, TypingForm::EnumNonmember))
}

/// Return type for [`collect_class_body`]: attributes, method names, and
/// per-method decorator lists.
pub(super) type ClassBodyInfo = (Vec<AttributeInfo>, Vec<String>, Vec<(String, Vec<String>)>);

use super::annotations::{
    annotation_contains_readonly_expr, annotation_is_class_var, annotation_is_final,
    annotation_is_init_var, annotation_is_kw_only,
};
use super::class_info_ext::{class_info_from, expr_simple_name};
use super::core::{classify_rhs, collect_function_scope, text_range_to_span};
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
    bindings: &BindingTable,
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
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
                if is_kw_only_sentinel(bindings, ann) {
                    after_kw_only_sentinel = true;
                } else if let Some(attr) =
                    ann_attribute(bindings, ann, after_kw_only_sentinel, None)
                {
                    attributes.push(attr);
                }
            }
            Stmt::Assign(assign) => {
                assign_attributes(bindings, assign, None, &mut attributes);
            }
            // A `if sys.version_info >= (...)` / `if TYPE_CHECKING` guard inside a
            // class body conditionally defines fields. Collect them, tagged with
            // the guard so `resolve_with_target` can prune the ones that do not
            // exist for the target version.
            Stmt::If(if_stmt) => {
                collect_guarded_fields(
                    bindings,
                    if_stmt,
                    None,
                    after_kw_only_sentinel,
                    &mut attributes,
                );
            }
            Stmt::FunctionDef(func) => {
                let func_info = function_info_from(bindings, func, Some(class.name.to_string()));
                let method_name = func_info.name.clone();
                let decs = func_info.decorators.clone();
                functions.push(func_info);
                // Any function collected from this method body is a closure
                // lexically nested inside the class; mark the non-method ones so
                // `generics_self_usage` does not treat their `Self` as module-level usage.
                let nested_start = functions.len();
                collect_function_scope(bindings, &func.body, functions, match_stmts);
                mark_nested_in_class(functions, nested_start);
                method_names.push(method_name.clone());
                method_decorators.push((method_name, decs));
            }
            Stmt::ClassDef(inner_class) => {
                // Nested classes contribute method FunctionInfos for annotation
                // checking; only module-level classes enter the module schema.
                let _inner_info = class_info_from(bindings, inner_class, functions, match_stmts);
            }
            _ => {}
        }
    }

    // A method can also be defined inside a compound statement in the class
    // body — the `if TYPE_CHECKING:` / `if sys.version_info >= (...)` /
    // `try: ... except ImportError:` compatibility shapes. The loop above only
    // sees the top level, and the `Stmt::If` arm collects fields but never
    // functions, so such a method was absent from `method_names` and its class
    // was reported as missing it. Attributes under the same guard were always
    // collected, which is what made the gap method-only.
    collect_nested_method_names(&class.body, &mut method_names, &mut method_decorators);

    (attributes, method_names, method_decorators)
}

/// Record methods defined inside compound statements in a class body.
///
/// Names only: the enclosing statement decides *whether* the definition runs,
/// which this resolver does not evaluate, so a guarded method is treated as
/// present rather than absent. Present-but-unanalysed can only cost a missed
/// diagnostic; absent produces a false positive on valid code, which is the
/// worse failure ([CHKARCH-CONFORMANCE]).
fn collect_nested_method_names(
    stmts: &[Stmt],
    method_names: &mut Vec<String>,
    method_decorators: &mut Vec<(String, Vec<String>)>,
) {
    for stmt in stmts {
        let nested: Vec<&[Stmt]> = match stmt {
            Stmt::If(node) => std::iter::once(node.body.as_slice())
                .chain(node.elif_else_clauses.iter().map(|c| c.body.as_slice()))
                .collect(),
            Stmt::Try(node) => std::iter::once(node.body.as_slice())
                .chain(node.handlers.iter().map(|handler| {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(handler) = handler;
                    handler.body.as_slice()
                }))
                .chain([node.orelse.as_slice(), node.finalbody.as_slice()])
                .collect(),
            Stmt::With(node) => vec![node.body.as_slice()],
            Stmt::For(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
            Stmt::While(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
            Stmt::Match(node) => node.cases.iter().map(|case| case.body.as_slice()).collect(),
            _ => continue,
        };

        for branch in nested {
            for inner in branch {
                if let Stmt::FunctionDef(func) = inner {
                    let name = func.name.to_string();
                    if !method_names.contains(&name) {
                        method_names.push(name.clone());
                        let decorators = func
                            .decorator_list
                            .iter()
                            .filter_map(decorator_name)
                            .collect();
                        method_decorators.push((name, decorators));
                    }
                }
            }
            // Guards nest — `if` inside `try`, `elif` chains, and so on.
            collect_nested_method_names(branch, method_names, method_decorators);
        }
    }
}

/// `true` for the keyword-only dataclass sentinel field.
fn is_kw_only_sentinel(bindings: &BindingTable, ann: &StmtAnnAssign) -> bool {
    matches!(expr_simple_name(&ann.target), Some(name) if name == "_")
        && annotation_is_kw_only(bindings, &ann.annotation)
}

/// Build an [`AttributeInfo`] from an annotated field (`name: T = value`), or
/// `None` when the target is not a simple name.
fn ann_attribute(
    bindings: &BindingTable,
    ann: &StmtAnnAssign,
    after_kw_only_sentinel: bool,
    guard: Option<StaticCondition>,
) -> Option<AttributeInfo> {
    let name = expr_simple_name(&ann.target)?;
    let field_kw_only = ann
        .value
        .as_deref()
        .and_then(|value| field_kw_only_override(bindings, value));
    // Determine kw_only: explicit field() override wins; then sentinel.
    let is_kw_only = field_kw_only.unwrap_or(after_kw_only_sentinel);
    Some(AttributeInfo {
        name,
        name_span: text_range_to_span(ann.target.range()),
        has_annotation: true,
        annotation_span: Some(text_range_to_span(ann.annotation.range())),
        has_value: ann.value.is_some(),
        rhs_kind: RhsKind::Other,
        rhs_span: ann.value.as_ref().map(|v| text_range_to_span(v.range())),
        rhs_is_lambda: false,
        rhs_descriptor: None,
        rhs_name: None,
        is_readonly: annotation_contains_readonly_expr(bindings, &ann.annotation),
        is_final: annotation_is_final(bindings, &ann.annotation),
        is_class_var: annotation_is_class_var(bindings, &ann.annotation),
        is_kw_only,
        is_init_false: ann
            .value
            .as_deref()
            .is_some_and(|value| field_init_is_false(bindings, value)),
        is_init_var: annotation_is_init_var(bindings, &ann.annotation),
        rhs_is_nonmember_call: ann
            .value
            .as_deref()
            .is_some_and(|value| rhs_is_nonmember_call(bindings, value)),
        guard,
    })
}

/// Classify a class-body assignment's RHS as a callable binding: the
/// descriptor wrapper (if any) and the simple name of the callable bound.
///
/// `m = f` → `(None, Some("f"))`; `s = staticmethod(g)` →
/// `(Some("staticmethod"), Some("g"))`; anything else → names absent ([#382]).
fn rhs_callable_binding(value: &Expr) -> (Option<String>, Option<String>) {
    match value {
        Expr::Name(name) => (None, Some(name.id.to_string())),
        Expr::Call(call) => {
            let wrapper = match call.func.as_ref() {
                Expr::Name(n) if n.id == "staticmethod" || n.id == "classmethod" => {
                    n.id.to_string()
                }
                _ => return (None, None),
            };
            let bound = match call.arguments.args.as_ref() {
                [Expr::Name(inner)] => Some(inner.id.to_string()),
                _ => None,
            };
            (Some(wrapper), bound)
        }
        _ => (None, None),
    }
}

/// Append an [`AttributeInfo`] for each simple-name target of `name = value`.
fn assign_attributes(
    bindings: &BindingTable,
    assign: &StmtAssign,
    guard: Option<&StaticCondition>,
    attributes: &mut Vec<AttributeInfo>,
) {
    let rhs_is_lambda = matches!(&*assign.value, Expr::Lambda(_));
    let (rhs_descriptor, rhs_name) = rhs_callable_binding(&assign.value);
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
                rhs_is_lambda,
                rhs_descriptor: rhs_descriptor.clone(),
                rhs_name: rhs_name.clone(),
                is_readonly: false,
                is_final: false,
                is_class_var: false,
                is_kw_only: false,
                is_init_false: false,
                is_init_var: false,
                rhs_is_nonmember_call: rhs_is_nonmember_call(bindings, &assign.value),
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
    bindings: &BindingTable,
    if_stmt: &StmtIf,
    outer: Option<&StaticCondition>,
    after_kw_only_sentinel: bool,
    attributes: &mut Vec<AttributeInfo>,
) {
    let test = parse_static_condition(bindings, &if_stmt.test);
    let if_guard = combine_guards(outer, test.clone());
    collect_fields_in_branch(
        bindings,
        &if_stmt.body,
        &if_guard,
        after_kw_only_sentinel,
        attributes,
    );

    // Each elif/else branch is reached only when every preceding test was false.
    let mut prior_negations = vec![StaticCondition::Not(Box::new(test))];
    for clause in &if_stmt.elif_else_clauses {
        let branch = match &clause.test {
            Some(elif_test) => {
                let cond = parse_static_condition(bindings, elif_test);
                let mut parts = prior_negations.clone();
                parts.push(cond.clone());
                prior_negations.push(StaticCondition::Not(Box::new(cond)));
                StaticCondition::All(parts)
            }
            None => StaticCondition::All(prior_negations.clone()),
        };
        let guard = combine_guards(outer, branch);
        collect_fields_in_branch(
            bindings,
            &clause.body,
            &guard,
            after_kw_only_sentinel,
            attributes,
        );
    }
}

/// Collect the field statements (and nested `if`s) of a single guarded branch.
fn collect_fields_in_branch(
    bindings: &BindingTable,
    stmts: &[Stmt],
    guard: &StaticCondition,
    after_kw_only_sentinel: bool,
    attributes: &mut Vec<AttributeInfo>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) if !is_kw_only_sentinel(bindings, ann) => {
                if let Some(attr) =
                    ann_attribute(bindings, ann, after_kw_only_sentinel, Some(guard.clone()))
                {
                    attributes.push(attr);
                }
            }
            Stmt::Assign(assign) => {
                assign_attributes(bindings, assign, Some(guard), attributes);
            }
            Stmt::If(nested) => {
                collect_guarded_fields(
                    bindings,
                    nested,
                    Some(guard),
                    after_kw_only_sentinel,
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
