//! Implements [`dataclasses_transform_class`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper types and functions for `dataclasses_transform_class`.
//!
//! Contains data types for transform-class settings and the four sub-checks
//! that back [`DataclassTransformClassViolation`]. Every setting is read from
//! the parsed `ruff` AST ([LINESCANPLAN-AST-MIGRATION], issue #408): decorator
//! keywords, class-header keywords, constructor call shapes, and comparison
//! expressions are structural facts, never re-parsed source lines.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;
use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Arguments, CmpOp, Expr, ModModule, Stmt, StmtClassDef};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::typing_form::dotted_spelling;
use crate::rules::shared::ExprIndex;

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_transform_class",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_transform_class",
};

/// Effective settings for a class that inherits from a `@dataclass_transform` base.
#[derive(Debug, Clone)]
pub(super) struct TransformClassSettings {
    /// Whether this class is effectively frozen.
    pub(super) frozen: bool,
    /// Whether this class has keyword-only constructor parameters.
    pub(super) kw_only: bool,
    /// Whether this class has ordering comparisons synthesised.
    pub(super) order: bool,
}

/// Defaults extracted from a `@dataclass_transform(...)` decorator on a class.
#[derive(Debug, Clone)]
pub(super) struct TransformBaseDefaults {
    pub(super) frozen_default: bool,
    pub(super) kw_only_default: bool,
    pub(super) order_default: bool,
}

/// The boolean value of keyword `key` in an argument list, when present and
/// literal.
fn bool_keyword(arguments: &Arguments, key: &str) -> Option<bool> {
    arguments.keywords.iter().find_map(|kw| {
        let matches_key = kw.arg.as_ref().is_some_and(|arg| arg.as_str() == key);
        match (&kw.value, matches_key) {
            (Expr::BooleanLiteral(lit), true) => Some(lit.value),
            _ => None,
        }
    })
}

/// Visit every class definition in the module, however nested.
fn for_each_class_def<'ast>(body: &'ast [Stmt], visit: &mut dyn FnMut(&'ast StmtClassDef)) {
    for stmt in body {
        match stmt {
            Stmt::ClassDef(class_def) => {
                visit(class_def);
                for_each_class_def(&class_def.body, visit);
            }
            Stmt::FunctionDef(func_def) => for_each_class_def(&func_def.body, visit),
            _ => {}
        }
    }
}

/// Find all class names decorated with `@dataclass_transform` (resolved
/// through the import cascade, bare or called) and parse their defaults from
/// the decorator call's keyword arguments.
pub(super) fn collect_transform_base_classes(
    resolver: &AnnotationResolver<'_>,
    ast: &ModModule,
) -> HashMap<String, TransformBaseDefaults> {
    let mut result = HashMap::new();
    for_each_class_def(&ast.body, &mut |class_def| {
        for decorator in &class_def.decorator_list {
            let (callee, arguments) = match &decorator.expression {
                Expr::Call(call) => (call.func.as_ref(), Some(&call.arguments)),
                other => (other, None),
            };
            let denotes_transform = dotted_spelling(callee).is_some_and(|spelling| {
                resolver.decorator_denotes(&spelling, "dataclass_transform")
            });
            if !denotes_transform {
                continue;
            }
            let defaults = TransformBaseDefaults {
                frozen_default: arguments
                    .and_then(|args| bool_keyword(args, "frozen_default"))
                    .unwrap_or(false),
                kw_only_default: arguments
                    .and_then(|args| bool_keyword(args, "kw_only_default"))
                    .unwrap_or(false),
                order_default: arguments
                    .and_then(|args| bool_keyword(args, "order_default"))
                    .unwrap_or(false),
            };
            let _ = result.insert(class_def.name.to_string(), defaults);
        }
    });
    result
}

/// For each class directly inheriting from a `@dataclass_transform` base,
/// compute its effective settings: the base's defaults overridden by the
/// class-header keywords (`class C(Base, frozen=True)`).
pub(super) fn collect_transform_subclasses(
    ast: &ModModule,
    transform_bases: &HashMap<String, TransformBaseDefaults>,
) -> HashMap<String, TransformClassSettings> {
    let mut result = HashMap::new();
    for_each_class_def(&ast.body, &mut |class_def| {
        let Some(arguments) = class_def.arguments.as_deref() else {
            return;
        };
        let base_defaults = arguments.args.iter().find_map(|base| match base {
            Expr::Name(name) => transform_bases.get(name.id.as_str()),
            _ => None,
        });
        let Some(base_defaults) = base_defaults else {
            return;
        };
        let settings = TransformClassSettings {
            frozen: bool_keyword(arguments, "frozen").unwrap_or(base_defaults.frozen_default),
            kw_only: bool_keyword(arguments, "kw_only").unwrap_or(base_defaults.kw_only_default),
            order: bool_keyword(arguments, "order").unwrap_or(base_defaults.order_default),
        };
        let _ = result.insert(class_def.name.to_string(), settings);
    });
    result
}

/// Compute the effective `frozen/kw_only/order` settings for a class that
/// **inherits from another transform subclass** (not directly from the base).
pub(super) fn resolve_inherited_settings(
    cls_name: &str,
    module: &ResolvedModule,
    direct_settings: &HashMap<String, TransformClassSettings>,
) -> Option<TransformClassSettings> {
    let mut visited = std::collections::HashSet::new();
    settings_walk(cls_name, module, direct_settings, &mut visited)
}

/// Recursive body of [`resolve_inherited_settings`]; `visited` breaks
/// base-name cycles (GitHub #278).
fn settings_walk<'a>(
    cls_name: &'a str,
    module: &'a ResolvedModule,
    direct_settings: &HashMap<String, TransformClassSettings>,
    visited: &mut std::collections::HashSet<&'a str>,
) -> Option<TransformClassSettings> {
    if !visited.insert(cls_name) {
        return None;
    }
    if let Some(s) = direct_settings.get(cls_name) {
        return Some(s.clone());
    }

    let cls = module.classes.iter().find(|c| c.name == cls_name)?;
    cls.bases
        .iter()
        .find_map(|base| settings_walk(base, module, direct_settings, visited))
}

/// Check 1: A non-frozen class directly inheriting from a frozen transform subclass.
pub(super) fn check_frozen_inheritance(
    module: &ResolvedModule,
    direct_settings: &HashMap<String, TransformClassSettings>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        if direct_settings.contains_key(cls.name.as_str()) {
            continue;
        }

        for base in &cls.bases {
            let Some(base_settings) = direct_settings.get(base.as_str()) else {
                continue;
            };
            if base_settings.frozen {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Non-frozen class `{}` cannot inherit from frozen \
                         dataclass-transform class `{}`",
                        cls.name, base
                    ),
                    cls.name_span,
                    path,
                    Some(
                        "A non-frozen class cannot subclass a frozen \
                         dataclass-transform class"
                            .to_owned(),
                    ),
                    Some(
                        "dataclass_transform: frozen and non-frozen classes \
                         cannot be mixed in the same hierarchy"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Check 2: Attribute assignment on a frozen transform-class instance.
pub(super) fn check_frozen_instance_assignment(
    module: &ResolvedModule,
    instance_map: &HashMap<&str, (&str, TransformClassSettings)>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for assign in &module.module_attr_assignments {
        let Some((class_name, settings)) = instance_map.get(assign.object_name.as_str()) else {
            continue;
        };
        if !settings.frozen {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot assign to attribute `{}` of frozen \
                 dataclass-transform class `{}` instance `{}`",
                assign.attr_name, class_name, assign.object_name
            ),
            assign.target_span,
            path,
            Some(
                "Instances of frozen dataclass-transform classes are immutable \
                 after construction"
                    .to_owned(),
            ),
            Some(
                "dataclass_transform(frozen=True) prohibits attribute assignment \
                 after construction"
                    .to_owned(),
            ),
        ));
    }
}

/// Check 3: Positional arguments to a `kw_only` transform-class constructor.
///
/// A module-level assignment whose RHS is a constructor call on a `kw_only`
/// transform class must pass every argument by keyword.
pub(super) fn check_kw_only_positional_args(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    direct_settings: &HashMap<String, TransformClassSettings>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(Expr::Call(call)) = index.expr(rhs_span) else {
            continue;
        };
        let Expr::Name(callee) = call.func.as_ref() else {
            continue;
        };
        let callee = callee.id.as_str();

        let Some(settings) = resolve_inherited_settings(callee, module, direct_settings) else {
            continue;
        };
        if !settings.kw_only {
            continue;
        }

        if !call.arguments.args.is_empty() {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Constructor of `{callee}` only accepts keyword arguments \
                     (kw_only=True)"
                ),
                rhs_span,
                path,
                Some(format!(
                    "Pass arguments as keyword arguments: `{callee}(field=value, ...)`"
                )),
                Some(
                    "dataclass_transform with kw_only_default=True makes all \
                     constructor parameters keyword-only"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Every ordering comparison (`<`, `>`, `<=`, `>=`) whose operand is a bare
/// name, with the comparison's source range.
struct OrderingComparisons<'ast> {
    hits: Vec<(&'ast str, basilisk_resolver::Span)>,
}

impl<'ast> Visitor<'ast> for OrderingComparisons<'ast> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Compare(compare) = expr {
            let is_ordering = compare
                .ops
                .iter()
                .any(|op| matches!(op, CmpOp::Lt | CmpOp::Gt | CmpOp::LtE | CmpOp::GtE));
            if is_ordering {
                let range = compare.range();
                let span = basilisk_resolver::Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                };
                for operand in
                    std::iter::once(compare.left.as_ref()).chain(compare.comparators.iter())
                {
                    if let Expr::Name(name) = operand {
                        self.hits.push((name.id.as_str(), span));
                    }
                }
            }
        }
        walk_expr(self, expr);
    }
}

/// Check 4: Comparison operator on a transform-class instance without `order=True`.
pub(super) fn check_no_order_comparison(
    ast: &ModModule,
    instance_map: &HashMap<&str, (&str, TransformClassSettings)>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if instance_map.is_empty() {
        return;
    }

    let mut visitor = OrderingComparisons { hits: Vec::new() };
    for stmt in &ast.body {
        visitor.visit_stmt(stmt);
    }

    let mut reported_spans = std::collections::HashSet::new();
    for (operand_name, span) in visitor.hits {
        let Some(&(class_name, ref settings)) = instance_map.get(operand_name) else {
            continue;
        };
        if settings.order {
            continue;
        }
        if !reported_spans.insert((span.start, span.end)) {
            continue; // One diagnostic per comparison expression.
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Comparison operator not supported: \
                 `{class_name}` does not synthesise ordering methods \
                 (order=False by default)"
            ),
            span,
            path,
            Some(format!(
                "Use `order=True` in `class {class_name}(...)` to enable ordering, \
                 or avoid `<`, `>`, `<=`, `>=` comparisons"
            )),
            Some(
                "dataclass_transform without order=True does not synthesise \
                 __lt__, __le__, __gt__, __ge__ methods"
                    .to_owned(),
            ),
        ));
    }
}
