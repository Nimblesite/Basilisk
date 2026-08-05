//! `classes_classvar`: `ClassVar` used in an invalid context.
//!
//! PEP 526 and the typing spec restrict `ClassVar[T]` to:
//!
//! - Annotations of class body attributes (class variables)
//!
//! Using `ClassVar` outside a class body (in function parameters, return types,
//! local variable annotations, or module-level variable annotations) is an error.
//! Additionally, nesting `ClassVar` inside another type constructor (e.g.
//! `Final[ClassVar[int]]` or `list[ClassVar[int]]`) is forbidden.
//!
//! Note: `Annotated[ClassVar[T], ...]` is a valid exception.
//!
//! This rule also validates `ClassVar` argument correctness:
//! - `ClassVar` accepts at most one argument
//! - The argument must be a valid type (not a literal or runtime variable)
//! - The argument must not contain `TypeVar`, `ParamSpec`, or `TypeVarTuple`
//!
//! Additionally, `ClassVar` attributes cannot be assigned via instances.
//!
//! Every verdict is structural over the parsed `ruff` AST, resolved through
//! the module's import cascade ([LINESCANPLAN-AST-MIGRATION]) — `ClassVar`,
//! `typing.ClassVar`, and any import alias all answer alike, and no check
//! reads annotation source text.
//!
//! ```python
//! class MyClass:
//!     bad9: Final[ClassVar[int]] = 3     # E0036 — ClassVar cannot be nested
//!     bad10: list[ClassVar[int]] = []    # E0036 — ClassVar cannot be nested
//!
//!     def method1(self, a: ClassVar[int]):   # E0036 — ClassVar not allowed here
//!         x: ClassVar[str] = ""              # E0036 — ClassVar not allowed here
//!         self.xx: ClassVar[str] = ""        # E0036 — ClassVar not allowed here
//!
//!     def method2(self) -> ClassVar[int]:    # E0036 — ClassVar not allowed here
//!         ...
//!
//! bad11: ClassVar[int] = 3              # E0036 — ClassVar not allowed at module level
//! bad12: TypeAlias = ClassVar[str]      # E0036 — ClassVar not allowed here
//! ```

mod args;
mod helpers;
mod instance;
mod protocol;

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::Diagnostic;
use crate::rules::shared::{runtime_value_names, type_constructor_names, ExprIndex};

use super::Rule;

use args::{check_classvar_args, check_classvar_init};
use helpers::{
    classvar_args, contains_classvar, has_nested_classvar, is_classvar, make_diagnostic,
    TypeParamKind,
};
use instance::{check_instance_classvar_assignments, check_self_classvar_annotations};
use protocol::check_protocol_classvar_conformance;

/// Emits `classes_classvar` for `ClassVar` used in an invalid context.
pub(crate) struct ClassVarInvalidContext;

impl Rule for ClassVarInvalidContext {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);

        let type_param_names = module_type_params(module);
        let runtime_names = runtime_value_names(module, &resolver, &index);
        let known_names = known_type_names(module);
        // A wildcard import can bind names this module cannot enumerate, so
        // "unbound" stops being provable.
        let has_star_import = module
            .imports
            .iter()
            .any(|import| import.names.iter().any(|name| name == "*"));
        let non_type = |name: &str| {
            runtime_names.contains(name)
                || (!has_star_import
                    && !known_names.contains(name)
                    && !resolver.is_grounded_name(name))
        };

        check_class_attributes(
            module,
            &resolver,
            &index,
            &type_param_names,
            &non_type,
            diagnostics,
        );
        check_function_contexts(module, &resolver, &index, diagnostics);
        check_self_classvar_annotations(module, &resolver, diagnostics);
        check_module_vars(module, &resolver, &index, diagnostics);
        check_instance_classvar_assignments(module, &resolver, &index, diagnostics);
        check_protocol_classvar_conformance(module, &resolver, &index, diagnostics);
    }
}

/// Every name this module binds — classes, functions, imports, variables,
/// and type constructors. A bare name outside this set (with no wildcard
/// import to hide behind) is unbound, and an unbound name is not a type.
fn known_type_names(module: &ResolvedModule) -> HashSet<String> {
    let mut names: HashSet<String> = module.classes.iter().map(|c| c.name.clone()).collect();
    names.extend(module.functions.iter().map(|f| f.name.clone()));
    names.extend(module.module_vars.iter().map(|v| v.name.clone()));
    names.extend(
        type_constructor_names(module)
            .into_iter()
            .map(str::to_owned),
    );
    for import in &module.imports {
        names.extend(import.names.iter().cloned());
        // `import x.y` binds the head `x`; `import x as z` binds `z` (already
        // in `names`).
        if let Some(head) = import.module.split('.').next() {
            let _ = names.insert(head.to_owned());
        }
    }
    names
}

/// Module-level `TypeVar` / `ParamSpec` / `TypeVarTuple` declarations, each
/// classified for error messaging.
fn module_type_params(module: &ResolvedModule) -> Vec<(String, TypeParamKind)> {
    module
        .typevar_calls
        .iter()
        .map(|tc| {
            let kind = if tc.is_paramspec {
                TypeParamKind::ParamSpec
            } else if tc.is_typevartuple {
                TypeParamKind::TypeVarTuple
            } else {
                TypeParamKind::TypeVar
            };
            (tc.name.clone(), kind)
        })
        .collect()
}

/// Class attributes: detect nested `ClassVar`, validate `ClassVar[...]`
/// arguments against the class's own type parameters, and check literal
/// initializers against the declared type.
fn check_class_attributes(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    type_param_names: &[(String, TypeParamKind)],
    non_type: &dyn Fn(&str) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        // Merge module-level type params with the class's own generic params.
        let class_params = cls.generic_params.iter().map(|gp| {
            let kind = if gp.is_typevartuple {
                TypeParamKind::TypeVarTuple
            } else {
                TypeParamKind::TypeVar
            };
            (gp.name.clone(), kind)
        });
        let all_type_params: Vec<(String, TypeParamKind)> = type_param_names
            .iter()
            .cloned()
            .chain(class_params)
            .collect();

        for attr in &cls.attributes {
            let Some(annotation) = attr.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };

            // Nested ClassVar (e.g. `Final[ClassVar[int]]`, `list[ClassVar[int]]`).
            if has_nested_classvar(resolver, annotation) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` cannot be nested inside another type in attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    &module.path,
                ));
            }

            // Argument validity for a top-level `ClassVar[...]`.
            if let Some(cv_args) = classvar_args(resolver, annotation) {
                check_classvar_args(
                    &cv_args,
                    &attr.name,
                    attr.name_span,
                    &module.path,
                    &all_type_params,
                    non_type,
                    diagnostics,
                );

                // PEP 526: a literal initializer must match the declared type.
                if let ([cv_arg], Some(rhs)) = (
                    cv_args.as_slice(),
                    attr.rhs_span.and_then(|span| index.expr(span)),
                ) {
                    check_classvar_init(
                        resolver,
                        cv_arg,
                        rhs,
                        &attr.name,
                        attr.name_span,
                        &module.path,
                        diagnostics,
                    );
                }
            }
        }
    }
}

/// Function contexts where `ClassVar` is never allowed: parameter
/// annotations, return annotations, and local variable annotations.
fn check_function_contexts(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let classvar_at = |span: Option<Span>| {
        span.and_then(|span| index.expr(span))
            .is_some_and(|expr| contains_classvar(resolver, expr))
    };
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if classvar_at(param.annotation_span) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` is not allowed in parameter annotation for `{}`",
                        param.name
                    ),
                    param.name_span,
                    &module.path,
                ));
            }
        }

        if classvar_at(func.return_annotation_span) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`ClassVar` is not allowed in the return annotation of `{}`",
                    func.name
                ),
                func.name_span,
                &module.path,
            ));
        }

        for var in &func.local_vars {
            if classvar_at(var.annotation_span) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` is not allowed in local variable annotation for `{}`",
                        var.name
                    ),
                    var.name_span,
                    &module.path,
                ));
            }
        }
    }
}

/// Module-level variables: `ClassVar` is not allowed in the annotation
/// (`bad11: ClassVar[int] = 3`) nor as an alias value
/// (`bad12: TypeAlias = ClassVar[str]`).
fn check_module_vars(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let annotated_classvar = var
            .annotation_span
            .and_then(|span| index.expr(span))
            .is_some_and(|expr| contains_classvar(resolver, expr));
        if annotated_classvar {
            diagnostics.push(make_diagnostic(
                format!(
                    "`ClassVar` is not allowed in module-level annotation for `{}`",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
            // Don't double-report for the same variable.
            continue;
        }
        let rhs_classvar = var
            .rhs_span
            .and_then(|span| index.expr(span))
            .is_some_and(|rhs| is_classvar(resolver, rhs));
        if rhs_classvar {
            diagnostics.push(make_diagnostic(
                format!(
                    "`ClassVar` is not allowed in right-hand side of module-level \
                     assignment for `{}`",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
        }
    }
}
