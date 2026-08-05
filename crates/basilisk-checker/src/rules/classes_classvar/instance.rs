//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Instance-level `ClassVar` violation checks for `classes_classvar`.
//!
//! Handles two cases, both over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION] — the previous byte scanner for `self.x:`
//! could not tell code from a docstring and hardcoded the fixture's `CV`
//! import alias):
//!
//! 1. `self.x: ClassVar[T]` annotations inside methods (invalid context).
//! 2. `instance.classvar_attr = value` assignments to class-level `ClassVar`
//!    attributes through an instance (forbidden by PEP 526).

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::ExprIndex;

use super::helpers::{is_classvar, make_diagnostic, CODE};

/// Every `self.<name>: <annotation>` target inside a method body, with the
/// attribute name, its span, and the annotation node.
struct SelfAnnotations<'ast> {
    hits: Vec<(String, Span, &'ast Expr)>,
    method_depth: usize,
    class_depth: usize,
}

impl<'ast> Visitor<'ast> for SelfAnnotations<'ast> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::ClassDef(StmtClassDef { body, .. }) => {
                self.class_depth += 1;
                for inner in body {
                    self.visit_stmt(inner);
                }
                self.class_depth -= 1;
            }
            Stmt::FunctionDef(StmtFunctionDef { body, .. }) => {
                // A function directly inside a class body is a method; the
                // `self.x` annotations that matter live in its body.
                let is_method = self.class_depth > 0;
                if is_method {
                    self.method_depth += 1;
                }
                for inner in body {
                    self.visit_stmt(inner);
                }
                if is_method {
                    self.method_depth -= 1;
                }
            }
            Stmt::AnnAssign(assign) if self.method_depth > 0 => {
                if let Expr::Attribute(attr) = assign.target.as_ref() {
                    if matches!(attr.value.as_ref(), Expr::Name(name) if name.id.as_str() == "self")
                    {
                        let range = attr.range();
                        self.hits.push((
                            attr.attr.to_string(),
                            Span {
                                start: range.start().to_u32(),
                                end: range.end().to_u32(),
                            },
                            &assign.annotation,
                        ));
                    }
                }
                walk_stmt(self, stmt);
            }
            other => walk_stmt(self, other),
        }
    }
}

/// Emit `classes_classvar` for every `self.<name>: ClassVar` annotation inside
/// a method body — these are not captured in `local_vars` because the
/// assignment target is an `Attribute` node rather than a `Name` node.
pub(super) fn check_self_classvar_annotations(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
        return;
    };
    let mut visitor = SelfAnnotations {
        hits: Vec::new(),
        method_depth: 0,
        class_depth: 0,
    };
    for stmt in &parsed.ast.body {
        visitor.visit_stmt(stmt);
    }
    for (attr_name, span, annotation) in &visitor.hits {
        if is_classvar(resolver, annotation) {
            diagnostics.push(make_diagnostic(
                format!("`ClassVar` is not allowed in self-attribute annotation for `{attr_name}`"),
                *span,
                &module.path,
            ));
        }
    }
}

/// Check module-level attribute assignments to `ClassVar`-annotated class
/// attributes: `enterprise_d.stats = {}` where `stats: ClassVar[...]`.
pub(super) fn check_instance_classvar_assignments(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let classvar_attrs: Vec<(&str, Vec<&str>)> = module
        .classes
        .iter()
        .filter_map(|cls| {
            let names: Vec<&str> = cls
                .attributes
                .iter()
                .filter(|attr| {
                    attr.annotation_span
                        .and_then(|span| index.expr(span))
                        .is_some_and(|ann| is_classvar(resolver, ann))
                })
                .map(|attr| attr.name.as_str())
                .collect();
            (!names.is_empty()).then_some((cls.name.as_str(), names))
        })
        .collect();
    if classvar_attrs.is_empty() {
        return;
    }

    let instances = instance_class_map(module, index);
    for assignment in &module.module_attr_assignments {
        let Some(class_name) = instances
            .iter()
            .find(|(var, _)| var == &assignment.object_name)
            .map(|(_, cls)| cls.as_str())
        else {
            // `Starship.stats = {}` assigns on the class itself, which is legal.
            continue;
        };
        let is_classvar_attr = classvar_attrs
            .iter()
            .any(|(cls, attrs)| *cls == class_name && attrs.contains(&assignment.attr_name.as_str()));
        if is_classvar_attr {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot assign to `ClassVar` attribute `{}` through an instance of `{}`",
                    assignment.attr_name, class_name
                ),
                assignment.target_span,
                &module.path,
                Some("Assign to the class directly instead: `ClassName.attr = value`".to_owned()),
                Some(
                    "PEP 526: ClassVar attributes can only be assigned on the class itself, \
                     not through instances"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Map module-level variable names to the class they are constructed from:
/// `enterprise_d = Starship(3000)` yields `("enterprise_d", "Starship")`.
/// Class-hood comes from the module's own class list, not from the name's
/// capitalisation.
fn instance_class_map(module: &ResolvedModule, index: &ExprIndex<'_>) -> Vec<(String, String)> {
    module
        .module_vars
        .iter()
        .filter_map(|var| {
            let Some(Expr::Call(call)) = var.rhs_span.and_then(|span| index.expr(span)) else {
                return None;
            };
            let Expr::Name(callee) = call.func.as_ref() else {
                return None;
            };
            let name = callee.id.as_str();
            module
                .classes
                .iter()
                .any(|cls| cls.name == name)
                .then(|| (var.name.clone(), name.to_owned()))
        })
        .collect()
}
