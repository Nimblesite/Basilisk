//! Implements [BSK-0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
//! BSK-0001: Missing parameter type annotation.
//! Implements the parameter-policy slice of [TYPEINF-REQUIRED] and the
//! receiver exemption shared by [TYPEINF-SPECIAL-SELF].
//!
//! Never fires where the current engine already infers the parameter type: a
//! scalar-literal default (`timeout=30` → `int`) determines the type, so
//! demanding an annotation there would be redundant ([TYPEINF-FUNC-DEFAULTS]).
//! Defaults that do NOT determine the type — `None`, empty containers, calls,
//! lambdas, arbitrary expressions — still require an annotation.
//!
//! ```python
//! def connect(timeout=30):              # ✓ — type inferred as int
//!     pass
//!
//! def connect(retries):                 # BSK-0001 — nothing to infer from
//!     pass
//!
//! def connect(timeout=None):            # BSK-0001 — None does not determine T | None
//!     pass
//! ```

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::inference::rhs_fully_determines_type;
use crate::param_infer::InferredParameters;
use crate::types::InferredType;

use super::shared::module_types::ModuleTypes;
use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0001",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0001",
};

/// Emits BSK-0001 for every unannotated regular parameter (not `*args`/`**kwargs`).
///
/// `*args` and `**kwargs` are handled by [`super::missing_vararg_annotation`].
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
// Implements [TYPEINF-FUNC-PARAMS] / [TYPEINF-EXCEEDS-REQUIRED] — every missing
// parameter annotation is a diagnostic, never a silent inference.
pub(crate) struct MissingParameterAnnotation;

impl Rule for MissingParameterAnnotation {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .functions
            .iter()
            .filter(|func| !is_stub_context(func, &module.classes))
            .for_each(|func| check_function(func, module, types, diagnostics));
    }
}

// Implements [TYPEINF-FUNC-SELFCLS] — only the first conventional receiver of
// an actual method is exempt. A free function parameter merely named `self` or
// `cls` has no implicit receiver type.
fn check_function(
    func: &FunctionInfo,
    module: &ResolvedModule,
    types: &ModuleTypes<'_>,
    out: &mut Vec<Diagnostic>,
) {
    // The engine's parameter inference runs at most once per function, and
    // only when some parameter would otherwise fire.
    let mut engine_inferred: Option<Option<InferredParameters>> = None;
    for (index, param) in func.parameters.iter().enumerate() {
        if param.has_annotation
            || is_implicit_receiver(func, index, param)
            || default_determines_type(param)
        {
            continue;
        }
        let inferred =
            engine_inferred.get_or_insert_with(|| engine_parameters(func, module, types));
        if parameter_inferable(inferred.as_ref(), &param.name) {
            continue;
        }
        out.push(make_diagnostic(param, &module.path));
    }
}

/// Implements [NARROWPLAN-INTEGRATION] Step 6 (issue #317): consult
/// [`crate::param_infer`] — body constraints plus same-module call sites —
/// before demanding an annotation the engine can already infer. Only
/// module-level functions are inferable; methods keep firing unchanged.
fn engine_parameters(
    func: &FunctionInfo,
    module: &ResolvedModule,
    types: &ModuleTypes<'_>,
) -> Option<InferredParameters> {
    if func.class_name.is_some() {
        return None;
    }
    let globals = engine_globals(module, types);
    let call_args = call_site_arguments(func, module, types);
    crate::param_infer::infer_parameters(&module.source, &func.name, &globals, &call_args)
}

/// The module's callables as engine globals: imported symbols plus every
/// module-level function with its DECLARED signature, resolved through the
/// shared cascade — never annotation text.
fn engine_globals(module: &ResolvedModule, types: &ModuleTypes<'_>) -> Vec<(String, InferredType)> {
    let mut globals = crate::param_infer::imported_callable_globals(module);
    let Some(resolver) = types.annotations() else {
        return globals;
    };
    let resolve = |span: Option<basilisk_resolver::Span>| {
        span.and_then(|span| resolver.resolve_span(span))
            .unwrap_or(InferredType::Unknown)
    };
    for function in module.functions.iter().filter(|f| f.class_name.is_none()) {
        let param_types = function
            .parameters
            .iter()
            .map(|parameter| resolve(parameter.annotation_span))
            .collect();
        globals.push((
            function.name.clone(),
            InferredType::Callable(crate::types::CallableInfo {
                param_types,
                return_type: Box::new(resolve(function.return_annotation_span)),
            }),
        ));
    }
    globals
}

/// The synthesized argument types of every same-module call to `func`, one
/// entry per call site — the call-site lower bounds of [`crate::param_infer`].
fn call_site_arguments(
    func: &FunctionInfo,
    module: &ResolvedModule,
    types: &ModuleTypes<'_>,
) -> Vec<Vec<InferredType>> {
    let Some(oracle) = types.oracle() else {
        return Vec::new();
    };
    module
        .calls
        .iter()
        .filter(|call| call.receiver.is_none() && call.callee == func.name)
        .map(|call| {
            call.args
                .iter()
                .map(|(_, span)| oracle.synth_span(*span).unwrap_or(InferredType::Unknown))
                .collect()
        })
        .collect()
}

/// Does the engine's inference pin this parameter to a fully-known type?
fn parameter_inferable(inferred: Option<&InferredParameters>, name: &str) -> bool {
    inferred.is_some_and(|params| {
        params.parameters.iter().any(|(param_name, ty)| {
            param_name == name
                && ty.as_ref().is_some_and(|ty| {
                    crate::expr_type::is_fully_known(ty)
                        && !matches!(ty, InferredType::Never | InferredType::Any)
                })
        })
    })
}

fn is_implicit_receiver(func: &FunctionInfo, index: usize, param: &ParameterInfo) -> bool {
    if index != 0
        || func.class_name.is_none()
        || super::shared::decorator_spelled(&func.decorators, "staticmethod")
    {
        return false;
    }
    let class_receiver = super::shared::decorator_spelled(&func.decorators, "classmethod")
        || matches!(func.name.as_str(), "__new__" | "__init_subclass__");
    param.name == if class_receiver { "cls" } else { "self" }
}

/// Implements [TYPEINF-FUNC-DEFAULTS]: `true` when the parameter's default
/// alone already tells the current engine the type — the annotation would be
/// redundant, so BSK-0001 must stay silent.
fn default_determines_type(param: &ParameterInfo) -> bool {
    param
        .default_rhs_kind
        .as_ref()
        .is_some_and(rhs_fully_determines_type)
}

fn make_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Missing parameter type annotation for `{}`", param.name),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `{}: <type>`", param.name)),
        Some(
            "Basilisk requires an explicit parameter type wherever it cannot be inferred; \
             a literal default (e.g. `timeout=30`) infers the type and needs no annotation"
                .to_owned(),
        ),
    )
}
