//! BSK-E0036: `ClassVar` used in an invalid context.
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

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

use args::{check_classvar_args, check_classvar_type_mismatch, extract_classvar_inner};
use helpers::{
    has_classvar, has_classvar_or_alias, has_nested_classvar, make_diagnostic, span_text,
    TypeParamKind,
};
use instance::{check_instance_classvar_assignments, check_self_classvar_annotations};
use protocol::check_protocol_classvar_conformance;

/// Emits BSK-E0036 for `ClassVar` used in an invalid context.
pub(crate) struct ClassVarInvalidContext;

impl Rule for ClassVarInvalidContext {
    #[expect(
        clippy::too_many_lines,
        reason = "ClassVar validation covers many distinct contexts"
    )]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect TypeVar/ParamSpec/TypeVarTuple names for ClassVar argument validation
        let type_param_names: Vec<(String, TypeParamKind)> = module
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
            .collect();

        // Collect module-level variable names for runtime variable detection
        let module_var_names: Vec<String> = module
            .module_vars
            .iter()
            .filter(|var| {
                // Exclude TypeVar/ParamSpec/TypeVarTuple assignments
                !type_param_names.iter().any(|(name, _)| name == &var.name)
            })
            .map(|var| var.name.clone())
            .collect();

        // --- Class attributes: detect nested ClassVar and validate arguments ---
        for cls in &module.classes {
            // Also collect generic params from the class itself
            let class_type_params: Vec<(String, TypeParamKind)> = cls
                .generic_params
                .iter()
                .map(|gp| {
                    let kind = if gp.is_typevartuple {
                        TypeParamKind::TypeVarTuple
                    } else {
                        TypeParamKind::TypeVar
                    };
                    (gp.name.clone(), kind)
                })
                .collect();

            // Merge module-level and class-level type params
            let all_type_params: Vec<(String, TypeParamKind)> = type_param_names
                .iter()
                .chain(class_type_params.iter())
                .cloned()
                .collect();

            for attr in &cls.attributes {
                let Some(ann) = span_text(source, attr.annotation_span) else {
                    continue;
                };

                // Check for nested ClassVar (e.g. Final[ClassVar[int]])
                if has_nested_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` cannot be nested inside another type in attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }

                // Validate ClassVar arguments and type mismatch
                if let Some(inner) = extract_classvar_inner(ann) {
                    check_classvar_args(
                        inner,
                        &attr.name,
                        attr.name_span,
                        path,
                        &all_type_params,
                        &module_var_names,
                        diagnostics,
                    );

                    // Check for type mismatch between ClassVar type and RHS value
                    if let Some(rhs_text) = span_text(source, attr.rhs_span) {
                        check_classvar_type_mismatch(
                            inner,
                            rhs_text,
                            &attr.name,
                            attr.name_span,
                            path,
                            diagnostics,
                        );
                    }
                }
            }
        }

        // --- Function parameters: ClassVar not allowed ---
        for func in &module.functions {
            for param in func
                .parameters
                .iter()
                .chain(func.vararg.iter())
                .chain(func.kwarg.iter())
            {
                let Some(ann) = span_text(source, param.annotation_span) else {
                    continue;
                };
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in parameter annotation for `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }

            // --- Function return type: ClassVar not allowed ---
            if let Some(ret_ann) = span_text(source, func.return_annotation_span) {
                if has_classvar(ret_ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in the return annotation of `{}`",
                            func.name
                        ),
                        func.name_span,
                        path,
                    ));
                }
            }

            // --- Local variables: ClassVar not allowed ---
            for var in &func.local_vars {
                let Some(ann) = span_text(source, var.annotation_span) else {
                    continue;
                };
                if has_classvar_or_alias(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in local variable annotation for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Self-attribute ClassVar annotations: scan source text ---
        // e.g. `self.xx: ClassVar[str] = ""`
        // These are not captured in local_vars because the target is an Attribute node.
        check_self_classvar_annotations(module, diagnostics);

        // --- Module-level variables: ClassVar not allowed ---
        for var in &module.module_vars {
            // Check annotation span (for `bad11: ClassVar[int] = 3`)
            if let Some(ann) = span_text(source, var.annotation_span) {
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in module-level annotation for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                    // Don't double-report for the same variable
                    continue;
                }
            }
            // Check RHS span (for `bad12: TypeAlias = ClassVar[str]`)
            if let Some(rhs) = span_text(source, var.rhs_span) {
                if has_classvar(rhs) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in right-hand side of module-level \
                             assignment for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Instance-level assignment to ClassVar attributes ---
        // e.g. `enterprise_d.stats = {}` where `stats` is ClassVar in the class
        check_instance_classvar_assignments(module, diagnostics);

        // --- Protocol ClassVar conformance ---
        // e.g. `a: ProtoA = ProtoAImpl()` where ProtoA requires ClassVar attrs
        check_protocol_classvar_conformance(module, diagnostics);
    }
}
