//! Implements [`typeddicts_operations`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `TypedDict` type consistency checks (PEP 589 §5).
//!
//! Validates assignments where the RHS is a `TypedDict`-typed variable:
//!
//! - `TypedDict` → `TypedDict`: structural compatibility check
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408).

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};
use ruff_python_ast::{Expr, ModModule, Operator, Stmt};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::typing_form::{strip_qualifiers, subscript_args};
use crate::rules::shared::{ann_str, ExprIndex};

use super::CODE;

/// Check `TypedDict` type consistency for module-level assignments.
pub(super) fn check_typeddict_assignability(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
        return;
    };
    let Some(resolver) = AnnotationResolver::for_module(module) else {
        return;
    };
    let index = ExprIndex::build(&parsed.ast);

    let td_classes: HashMap<&str, &ClassInfo> = module
        .classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| (c.name.as_str(), c))
        .collect();

    if td_classes.is_empty() {
        return;
    }

    let ctx = TdContext {
        resolver: &resolver,
        index: &index,
        ast: &parsed.ast,
        td_classes: &td_classes,
    };
    let var_td_types = build_var_typeddict_map(module, &ctx);

    for var in &module.module_vars {
        // RHS must be a simple variable name referencing a TypedDict-typed var.
        let Some(Expr::Name(rhs)) = var.rhs_span.and_then(|span| ctx.index.expr(span)) else {
            continue;
        };
        let Some(&rhs_td_name) = var_td_types.get(rhs.id.as_str()) else {
            continue;
        };

        // The LHS annotation — from this statement or a prior declaration.
        let Some(annotation) = var.annotation_span.and_then(|span| ctx.index.expr(span)) else {
            // Reassignment: the variable keeps its originally declared
            // TypedDict type, which assigning another TypedDict never violates
            // unless the two are structurally incompatible.
            if let Some(td_name) = var_td_types.get(var.name.as_str()) {
                check_td_to_td(
                    &ctx,
                    td_name,
                    rhs_td_name,
                    var.name_span,
                    module,
                    diagnostics,
                );
            }
            continue;
        };

        check_td_to_target(
            &ctx,
            annotation,
            rhs_td_name,
            var.name_span,
            module,
            diagnostics,
        );
    }
}

/// Everything a `TypedDict` consistency verdict needs.
struct TdContext<'m, 'ast> {
    resolver: &'m AnnotationResolver<'m>,
    index: &'m ExprIndex<'ast>,
    ast: &'ast ModModule,
    td_classes: &'m HashMap<&'m str, &'m ClassInfo>,
}

/// Check an assignment where the RHS is a `TypedDict` variable.
fn check_td_to_target(
    ctx: &TdContext<'_, '_>,
    annotation: &Expr,
    rhs_td_name: &str,
    span: basilisk_resolver::Span,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // TypedDict → TypedDict: structural compatibility.
    if let Expr::Name(lhs_name) = annotation {
        check_td_to_td(
            ctx,
            lhs_name.id.as_str(),
            rhs_td_name,
            span,
            module,
            diagnostics,
        );
    }
}

/// Structural compatibility between two named `TypedDict`s.
fn check_td_to_td(
    ctx: &TdContext<'_, '_>,
    lhs_td_name: &str,
    rhs_td_name: &str,
    span: basilisk_resolver::Span,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if lhs_td_name == rhs_td_name {
        return;
    }
    let (Some(lhs_cls), Some(rhs_cls)) = (
        ctx.td_classes.get(lhs_td_name),
        ctx.td_classes.get(rhs_td_name),
    ) else {
        return;
    };
    if let Some(detail) = structural_incompatibility(ctx, lhs_cls, rhs_cls) {
        emit_td_error(
            diagnostics,
            span,
            &module.path,
            &format!("TypedDict `{rhs_td_name}` is not assignable to `{lhs_td_name}`: {detail}"),
            "TypedDict types use structural compatibility with invariant value types",
        );
    }
}

/// Emit a `TypedDict` assignability error.
fn emit_td_error(
    diagnostics: &mut Vec<Diagnostic>,
    span: basilisk_resolver::Span,
    path: &str,
    message: &str,
    help: &str,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        message.to_owned(),
        span,
        path,
        Some(help.to_owned()),
        Some("PEP 589: TypedDict type consistency rules".to_owned()),
    ));
}

/// Map each annotated module variable to the `TypedDict` class it is typed as.
fn build_var_typeddict_map<'m>(
    module: &'m ResolvedModule,
    ctx: &TdContext<'m, '_>,
) -> HashMap<&'m str, &'m str> {
    let mut map = HashMap::new();
    for var in &module.module_vars {
        let Some(Expr::Name(ann)) = var.annotation_span.and_then(|span| ctx.index.expr(span))
        else {
            continue;
        };
        if let Some(cls) = ctx.td_classes.get(ann.id.as_str()) {
            let _ = map.insert(var.name.as_str(), cls.name.as_str());
        }
    }
    map
}

/// `true` when `td_name` declares `extra_items=` (PEP 728) and every value
/// type of the `TypedDict` — field annotations plus the extra-items type — is
/// assignable to `target_value_type` (rendered).
fn extra_items_values_assignable(
    ctx: &TdContext<'_, '_>,
    td_name: &str,
    target_value_type: &str,
) -> bool {
    let Some(cls) = ctx.td_classes.get(td_name) else {
        return false;
    };
    let Some(extra_type) = class_keyword_value(ctx.ast, &cls.name, "extra_items") else {
        return false;
    };
    let mut members: Vec<&Expr> = vec![strip_qualifiers(ctx.resolver, extra_type)];
    for attr in &cls.attributes {
        let Some(ann) = attr.annotation_span.and_then(|span| ctx.index.expr(span)) else {
            return false;
        };
        members.push(strip_qualifiers(ctx.resolver, ann));
    }
    members
        .iter()
        .all(|member| crate::rules::shared::is_type_compatible(&ann_str(member), target_value_type))
}

/// The value expression of the class-definition keyword `keyword` on the class
/// named `class_name`, found by walking the parsed module — the structural
/// replacement for scanning the class-header text for `keyword=`.
fn class_keyword_value<'ast>(
    ast: &'ast ModModule,
    class_name: &str,
    keyword: &str,
) -> Option<&'ast Expr> {
    fn walk<'ast>(body: &'ast [Stmt], class_name: &str, keyword: &str) -> Option<&'ast Expr> {
        for stmt in body {
            match stmt {
                Stmt::ClassDef(class_def) => {
                    if class_def.name.as_str() == class_name {
                        if let Some(arguments) = class_def.arguments.as_deref() {
                            for kw in &*arguments.keywords {
                                if kw.arg.as_ref().is_some_and(|arg| arg.as_str() == keyword) {
                                    return Some(&kw.value);
                                }
                            }
                        }
                        return None;
                    }
                    if let Some(found) = walk(&class_def.body, class_name, keyword) {
                        return Some(found);
                    }
                }
                Stmt::FunctionDef(func_def) => {
                    if let Some(found) = walk(&func_def.body, class_name, keyword) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&ast.body, class_name, keyword)
}

/// One structural field: name, value-type node (qualifiers peeled), and
/// required-ness as declared by the class's totality.
struct TdField<'m, 'ast> {
    name: &'m str,
    value_type: &'ast Expr,
    required: bool,
}

/// Extract each field's name, value type, and required-ness structurally.
fn extract_fields<'m, 'ast>(
    ctx: &TdContext<'m, 'ast>,
    cls: &'m ClassInfo,
) -> Vec<TdField<'m, 'ast>> {
    cls.attributes
        .iter()
        .filter_map(|attr| {
            let ann = attr.annotation_span.and_then(|span| ctx.index.expr(span))?;
            Some(TdField {
                name: attr.name.as_str(),
                value_type: strip_qualifiers(ctx.resolver, ann),
                required: cls.is_typeddict_total,
            })
        })
        .collect()
}

/// The first structural incompatibility between two `TypedDict`s, if any.
fn structural_incompatibility(
    ctx: &TdContext<'_, '_>,
    lhs: &ClassInfo,
    rhs: &ClassInfo,
) -> Option<String> {
    let lhs_fields = extract_fields(ctx, lhs);
    let rhs_fields = extract_fields(ctx, rhs);

    let rhs_map: HashMap<&str, &TdField<'_, '_>> =
        rhs_fields.iter().map(|field| (field.name, field)).collect();

    for lhs_field in &lhs_fields {
        let Some(rhs_field) = rhs_map.get(lhs_field.name) else {
            return Some(format!("missing key `{}`", lhs_field.name));
        };

        if !types_structurally_equal(ctx, lhs_field.value_type, rhs_field.value_type) {
            return Some(format!(
                "value type for key `{}` is `{}`, expected `{}`",
                lhs_field.name,
                ann_str(rhs_field.value_type),
                ann_str(lhs_field.value_type)
            ));
        }

        if lhs_field.required != rhs_field.required {
            let (ls, rs) = if lhs_field.required {
                ("required", "non-required")
            } else {
                ("non-required", "required")
            };
            return Some(format!(
                "key `{}` is {rs} in source but {ls} in target",
                lhs_field.name
            ));
        }
    }

    None
}

/// Are two value-type expressions structurally equal — same rendered form,
/// equivalent `TypedDict` structure, or component-wise equal unions?
fn types_structurally_equal(ctx: &TdContext<'_, '_>, lhs: &Expr, rhs: &Expr) -> bool {
    // Same structure regardless of source formatting.
    if ann_str(lhs) == ann_str(rhs) {
        return true;
    }

    // Both TypedDict names: structural equivalence.
    if let (Expr::Name(lhs_name), Expr::Name(rhs_name)) = (lhs, rhs) {
        if let (Some(lhs_cls), Some(rhs_cls)) = (
            ctx.td_classes.get(lhs_name.id.as_str()),
            ctx.td_classes.get(rhs_name.id.as_str()),
        ) {
            return structural_incompatibility(ctx, lhs_cls, rhs_cls).is_none();
        }
    }

    // Unions compare component-wise, position by position.
    if let (Expr::BinOp(lhs_op), Expr::BinOp(rhs_op)) = (lhs, rhs) {
        if lhs_op.op == Operator::BitOr && rhs_op.op == Operator::BitOr {
            return types_structurally_equal(ctx, &lhs_op.left, &rhs_op.left)
                && types_structurally_equal(ctx, &lhs_op.right, &rhs_op.right);
        }
    }

    false
}
