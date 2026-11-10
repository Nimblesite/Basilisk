//! Implements [`calls_argument_type`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `calls_argument_type`: Argument type mismatch at a call site.
//!
//! Every argument is judged by the TYPE the module's bidirectional engine
//! synthesises for it ([NARROWPLAN-INTEGRATION] Step 3), checked against the
//! declared parameter type through the one shared judgment
//! ([`TypeJudge`]) — never by the syntactic shape of the expression.
//!
//! ```python
//! def add(x: int, y: int) -> int:
//!     return x + y
//!
//! result: int = add("hello", "world")   # str literals for int params → E0012
//! ```

mod arg_types;
mod builtin_methods;

use std::collections::HashMap;

use basilisk_resolver::{CallSite, FunctionInfo, ResolvedModule, Span, TypeVarCallInfo};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::judge::TypeJudge;
use crate::rules::shared::{is_type_compatible, parse_subscript_annotation};
use crate::span_util::slice_span;
use crate::types::{InferredType, LiteralValue};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "calls_argument_type",
    docs_url: "https://www.basilisk-python.dev/errors/calls_argument_type",
};

/// Emits `calls_argument_type` for call sites where a literal argument is incompatible
/// with the declared parameter type.
pub(crate) struct ArgumentTypeMismatch;

impl Rule for ArgumentTypeMismatch {
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
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(resolver) = types.annotations() else {
            return;
        };
        let judge = TypeJudge::new(types.oracle(), resolver, types.subtyping());
        check_local_function_calls(module, resolver, &judge, diagnostics);
        builtin_methods::check_builtin_method_argument_types(module, &judge, diagnostics);
    }
}

/// Judge every argument of every call to a module-level function.
fn check_local_function_calls(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    judge: &TypeJudge<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let func_groups = group_module_functions(module);
    // TypeVar bounds/constraints, used to detect calls for which no TypeVar
    // assignment exists (e.g. a `list[T_int]` parameter given `list[str]`).
    let typevars: HashMap<&str, &TypeVarCallInfo> = module
        .typevar_calls
        .iter()
        .map(|tv| (tv.name.as_str(), tv))
        .collect();

    for call in &module.calls {
        // Bound calls are checked against receiver-aware declarations by the
        // builtin-method pass, never against a same-named module function.
        if call.receiver.is_some() {
            continue;
        }
        let Some(funcs) = func_groups.get(call.callee.as_str()) else {
            continue;
        };
        let Some(func) = resolve_overload_for_call(funcs) else {
            continue;
        };
        check_call_arguments(module, call, func, resolver, judge, &typevars, diagnostics);
    }
}

/// Group module-level functions by name → list of overloads/implementations.
fn group_module_functions(module: &ResolvedModule) -> HashMap<&str, Vec<&FunctionInfo>> {
    let mut func_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
    for func in &module.functions {
        if func.class_name.is_none() {
            func_groups
                .entry(func.name.as_str())
                .or_default()
                .push(func);
        }
    }
    func_groups
}

/// Judge each positional argument of `call` against `func`'s declared
/// parameter types.
///
/// A callee with `*args` breaks the positional zip — arguments past the
/// prefix belong to the vararg, and `FunctionInfo.parameters` mixes
/// keyword-only parameters into the same list — so such callees are not
/// judged positionally at all ([CHKARCH-CONFORMANCE-MODE]).
fn check_call_arguments(
    module: &ResolvedModule,
    call: &CallSite,
    func: &FunctionInfo,
    resolver: &AnnotationResolver<'_>,
    judge: &TypeJudge<'_, '_>,
    typevars: &HashMap<&str, &TypeVarCallInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if func.vararg.is_some() {
        return;
    }
    for (arg_idx, (_, arg_span)) in call.args.iter().enumerate() {
        let Some(param) = func.parameters.get(arg_idx) else {
            break;
        };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(&module.source, ann_span) else {
            continue;
        };
        let mismatch = argument_mismatch(judge, resolver, ann_span, ann_text, *arg_span, typevars);
        if let Some(description) = mismatch {
            diagnostics.push(make_diagnostic(
                &call.callee,
                &param.name,
                ann_text,
                &description,
                *arg_span,
                &module.path,
            ));
        }
    }
}

/// The description of a proven mismatch between the argument the engine
/// typed and the parameter's declared type, or `None` when the argument
/// fits or the evidence is incomplete ([CHKARCH-CONFORMANCE-MODE]).
fn argument_mismatch(
    judge: &TypeJudge<'_, '_>,
    resolver: &AnnotationResolver<'_>,
    ann_span: Span,
    ann_text: &str,
    arg_span: Span,
    typevars: &HashMap<&str, &TypeVarCallInfo>,
) -> Option<String> {
    let inferred = judge.inferred(Some(arg_span));
    if let Some(description) = container_mismatch(ann_text, &inferred, typevars) {
        return Some(description);
    }
    if matches!(inferred, InferredType::Unknown | InferredType::Any) {
        return None;
    }
    let declared = resolver.resolve_span(ann_span)?;
    let silent = judge.fits(&inferred, &declared)
        || judge.display_checks(Some(arg_span), &declared)
        || !judge.judgeable(&declared)
        || !judge.evidence(&inferred)
        || !deeply_grounded(resolver, &declared);
    if silent {
        return None;
    }
    Some(format!("`{inferred}`"))
}

/// Is every leaf of `declared` a type this module can rule on? A `TypeVar`
/// spelled as a name (`list[T]`), an unresolved import, or a structural
/// marker anywhere inside the annotation makes the whole parameter a
/// question, not an answer — the judgment abstains rather than guessing
/// ([CHKARCH-CONFORMANCE-MODE]).
fn deeply_grounded(resolver: &AnnotationResolver<'_>, declared: &InferredType) -> bool {
    match declared {
        InferredType::Named(name) => resolver.is_grounded_name(name),
        InferredType::List(element)
        | InferredType::Set(element)
        | InferredType::Optional(element) => deeply_grounded(resolver, element),
        InferredType::Dict(key, value) => {
            deeply_grounded(resolver, key) && deeply_grounded(resolver, value)
        }
        InferredType::Tuple(elements) | InferredType::Union(elements) => elements
            .iter()
            .all(|element| deeply_grounded(resolver, element)),
        // Parameter positions carry variance this judgment does not model,
        // and a `TypeForm` parameter accepts type EXPRESSIONS — strings
        // included (PEP 747) — which need type-form evaluation, not value
        // judgment.
        InferredType::Callable(_)
        | InferredType::Generator(..)
        | InferredType::Guard { .. }
        | InferredType::TypeForm(_) => false,
        _ => true,
    }
}

/// The one function signature to check arguments against.
///
/// A name bound to several declarations is an overload group; choosing among
/// its members is overload resolution, which this rule does not perform, so
/// the judgment abstains rather than guess ([CHKARCH-CONFORMANCE-MODE]).
fn resolve_overload_for_call<'a>(funcs: &[&'a FunctionInfo]) -> Option<&'a FunctionInfo> {
    if funcs.len() <= 1 {
        return funcs.first().copied();
    }
    None
}

/// A container parameter (`list[...]`, `set[...]`, …) that no `TypeVar`
/// assignment can satisfy: either a positively-known scalar argument, or a
/// container whose known element type violates the parameter's `TypeVar`
/// bound/constraints.
///
/// Implements the typing-spec rule that a call is an error when the collected
/// constraints for a type variable have no common solution
/// ([CHKARCH-DIAG-TYPESAFETY]).
fn container_mismatch(
    annotation: &str,
    inferred: &InferredType,
    typevars: &HashMap<&str, &TypeVarCallInfo>,
) -> Option<String> {
    let (base, args) = parse_subscript_annotation(annotation)?;
    let base = base.trim().to_ascii_lowercase();
    if !matches!(
        base.as_str(),
        "list" | "set" | "frozenset" | "dict" | "tuple"
    ) {
        return None;
    }

    // (a) A scalar value can never satisfy a container parameter, whatever the
    //     element type — no assignment of any TypeVar makes it valid.
    if scalar_type_name(inferred).is_some() || matches!(inferred, InferredType::None_) {
        return Some(format!(
            "`{inferred}` where `{annotation}` is required — no type-variable \
             assignment makes it valid"
        ));
    }

    // (b) An invariant container of a single bounded/constrained TypeVar, given
    //     an argument whose known element type violates the bound/constraints.
    if matches!(base.as_str(), "list" | "set" | "frozenset") {
        let inner = args.first()?;
        let tv = typevars.get(inner.as_str())?;
        let elem = known_element_type(inferred)?;
        if !typevar_accepts(tv, elem) {
            return Some(format!(
                "`{base}[{elem}]` where `{annotation}` is required — `{elem}` does not \
                 satisfy type variable `{inner}`"
            ));
        }
    }
    None
}

/// Does the `TypeVar`'s bound or constraint set admit `elem`?
fn typevar_accepts(tv: &TypeVarCallInfo, elem: &str) -> bool {
    match &tv.bound_type_name {
        Some(bound) => is_type_compatible(elem, bound),
        None if !tv.constraint_type_names.is_empty() => tv
            .constraint_type_names
            .iter()
            .any(|constraint| is_type_compatible(elem, constraint)),
        None => true,
    }
}

/// The element type name of a `list`/`set` argument whose engine-synthesised
/// element type is a known scalar (`[""]` → `str`); `None` otherwise.
fn known_element_type(inferred: &InferredType) -> Option<&'static str> {
    let (InferredType::List(element) | InferredType::Set(element)) = inferred else {
        return None;
    };
    scalar_type_name(element)
}

/// The Python type name of a positively-known scalar type.
fn scalar_type_name(inferred: &InferredType) -> Option<&'static str> {
    match inferred {
        InferredType::Int => Some("int"),
        InferredType::Float => Some("float"),
        InferredType::Str | InferredType::LiteralString => Some("str"),
        InferredType::Bool => Some("bool"),
        InferredType::Bytes => Some("bytes"),
        InferredType::Literal(value) => Some(match value {
            LiteralValue::Int(_) => "int",
            LiteralValue::Float(_) => "float",
            LiteralValue::Str(_) => "str",
            LiteralValue::Bool(_) => "bool",
            LiteralValue::Bytes(_) => "bytes",
        }),
        InferredType::Union(members) => {
            let first = scalar_type_name(members.first()?)?;
            members
                .iter()
                .all(|member| scalar_type_name(member) == Some(first))
                .then_some(first)
        }
        _ => None,
    }
}

pub(super) fn make_diagnostic(
    callee: &str,
    param_name: &str,
    annotation: &str,
    rhs_description: &str,
    span: Span,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument `{param_name}` of `{callee}` expects `{annotation}` but received \
             {rhs_description}"
        ),
        span,
        path,
        Some(format!(
            "Pass a value of type `{annotation}` for parameter `{param_name}`"
        )),
        Some(
            "Basilisk checks that literal arguments are compatible with declared parameter types"
                .to_owned(),
        ),
    )
}
