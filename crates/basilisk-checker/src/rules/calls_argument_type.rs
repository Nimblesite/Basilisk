//! Implements [`calls_argument_type`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! `calls_argument_type`: Argument type mismatch at a call site.
//!
//! When a function is called with a literal argument whose type is clearly
//! incompatible with the declared parameter annotation, Basilisk reports the
//! mismatch.  The check mirrors the literal-kind vs annotation comparison
//! used by `assignment_compatibility`.
//!
//! ```python
//! def add(x: int, y: int) -> int:
//!     return x + y
//!
//! result: int = add("hello", "world")   # str literals for int params → E0012
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind, Span, TypeVarCallInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{is_type_compatible, parse_subscript_annotation};
use crate::span_util::slice_span;

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
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Group module-level functions by name → list of overloads/implementations.
        let mut func_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                func_groups
                    .entry(func.name.as_str())
                    .or_default()
                    .push(func);
            }
        }

        // TypeVar bounds/constraints, used to detect calls for which no TypeVar
        // assignment exists (e.g. a `list[T_int]` parameter given `list[str]`).
        let typevars: HashMap<&str, &TypeVarCallInfo> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv))
            .collect();

        for call in &module.calls {
            // Only check calls to locally-defined functions for now.
            // Cross-module argument checking requires parsing imported function
            // signatures from `ExternalSymbol` — future work (Phase 4+).
            let Some(funcs) = func_groups.get(call.callee.as_str()) else {
                continue;
            };

            // Determine which function to check arguments against.
            let func_to_check = resolve_overload_for_call(funcs, call.args.len(), module);

            let Some(func) = func_to_check else {
                continue;
            };

            for (arg_idx, (rhs_kind, arg_span)) in call.args.iter().enumerate() {
                let Some(param) = func.parameters.get(arg_idx) else {
                    break;
                };

                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(&module.source, ann_span) else {
                    continue;
                };

                let arg_source = slice_span(&module.source, *arg_span);

                let mismatch = container_mismatch(ann_text, rhs_kind, &typevars).or_else(|| {
                    arg_rhs_mismatch(ann_text, rhs_kind, arg_source).map(str::to_owned)
                });
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
        check_builtin_method_argument_types(module, diagnostics);
    }
}

/// Validate literal arguments to bound built-in methods against all applicable
/// overloads from the active `builtins.pyi` declaration ([STUBRES-PYI] #288).
fn check_builtin_method_argument_types(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    for call in &module.calls {
        let declarations: Vec<_> = module
            .builtin_methods_for_call(call)
            .into_iter()
            .filter(|declaration| {
                super::calls_argument_count::stub_arity_accepts(declaration, call.args.len())
            })
            .collect();
        if declarations.is_empty() {
            continue;
        }
        if declarations.iter().any(|declaration| {
            call.args.iter().enumerate().all(|(index, (rhs, _))| {
                declaration
                    .params
                    .get(index)
                    .and_then(|parameter| parameter.annotation.as_deref())
                    .is_none_or(|annotation| stub_argument_compatible(annotation, rhs))
            })
        }) {
            continue;
        }
        let Some((index, (rhs, span))) = call.args.iter().enumerate().find(|(index, (rhs, _))| {
            declarations.iter().all(|declaration| {
                declaration
                    .params
                    .get(*index)
                    .and_then(|parameter| parameter.annotation.as_deref())
                    .is_some_and(|annotation| !stub_argument_compatible(annotation, rhs))
            })
        }) else {
            continue;
        };
        let expected = declarations
            .iter()
            .filter_map(|declaration| declaration.params.get(index))
            .filter_map(|parameter| parameter.annotation.as_deref())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(" | ");
        let description = format!("an argument incompatible with `{expected}` ({rhs:?})");
        diagnostics.push(make_diagnostic(
            &call.callee,
            declarations
                .first()
                .and_then(|declaration| declaration.params.get(index))
                .map_or("argument", |parameter| parameter.name.as_str()),
            &expected,
            &description,
            *span,
            &module.path,
        ));
    }
}

fn stub_argument_compatible(annotation: &str, rhs: &RhsKind) -> bool {
    let normalized = annotation.replace(' ', "");
    if normalized == "Any" || normalized == "object" {
        return true;
    }
    if normalized.contains("Iterable[str]") || normalized.contains("Iterable[LiteralString]") {
        return match rhs {
            RhsKind::StrLiteral
            | RhsKind::EmptyList
            | RhsKind::Other
            | RhsKind::CallExpr
            | RhsKind::KnownCall(_) => true,
            RhsKind::List(items) | RhsKind::Tuple(items) | RhsKind::Set(items) => {
                items.iter().all(|item| matches!(item, RhsKind::StrLiteral))
            }
            _ => false,
        };
    }
    if normalized.contains("LiteralString") {
        return matches!(
            rhs,
            RhsKind::StrLiteral | RhsKind::Other | RhsKind::CallExpr
        );
    }
    arg_rhs_mismatch(annotation, rhs, None).is_none()
}

/// For an overloaded function, determine which function signature to check
/// arguments against.
///
/// If there is only one function (no overloads), returns it directly.
/// If there are overloads, filters by arity — if exactly one overload matches
/// the provided argument count, returns that overload. Otherwise returns the
/// implementation (non-overload) function.
fn resolve_overload_for_call<'a>(
    funcs: &[&'a FunctionInfo],
    arg_count: usize,
    module: &ResolvedModule,
) -> Option<&'a FunctionInfo> {
    if funcs.len() <= 1 {
        return funcs.first().copied();
    }

    // Separate overload stubs from the implementation.
    let overloads: Vec<&FunctionInfo> = funcs
        .iter()
        .filter(|f| is_overload_stub(f, module))
        .copied()
        .collect();

    if overloads.is_empty() {
        // No @overload decorators — just use the last function.
        return funcs.last().copied();
    }

    // Filter overloads by arity: keep those where the argument count is valid.
    let arity_matches: Vec<&FunctionInfo> = overloads
        .iter()
        .filter(|f| {
            // Skip functions with *args (they accept any number of positional args)
            if f.vararg.is_some() {
                return true;
            }
            let required = f.parameters.iter().filter(|p| !p.has_default).count();
            let total = f.parameters.len();
            arg_count >= required && arg_count <= total
        })
        .copied()
        .collect();

    match arity_matches.len() {
        0 => {
            // No overload matches arity — fall back to implementation.
            funcs.iter().find(|f| !is_overload_stub(f, module)).copied()
        }
        1 => {
            // Exactly one overload matches arity — check against it.
            arity_matches.first().copied()
        }
        _ => {
            // Multiple overloads match arity — fall back to implementation
            // to avoid false positives (full type-based overload resolution
            // would be needed to pick the right one).
            funcs.iter().find(|f| !is_overload_stub(f, module)).copied()
        }
    }
}

/// Returns `true` if the function has an `@overload` decorator and a stub body.
fn is_overload_stub(func: &FunctionInfo, _module: &ResolvedModule) -> bool {
    func.is_stub_body
        && func
            .decorators
            .iter()
            .any(|d| d == "overload" || d.ends_with(".overload"))
}

/// Returns a human-readable description when `rhs` is incompatible with
/// the annotation text, or `None` when the pairing is acceptable.
///
/// `arg_source` is the raw source text of the argument expression, used to
/// disambiguate `CallExpr` arguments (e.g. detecting `type(None)` vs other calls).
fn arg_rhs_mismatch(
    annotation: &str,
    rhs: &RhsKind,
    arg_source: Option<&str>,
) -> Option<&'static str> {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    // A `*tuple[Any, ...]` parameter (PEP 646) accepts any variadic sequence,
    // so an argument's runtime-determined type is not an E0012 mismatch. Arity
    // and shape errors for TypeVarTuple parameters are handled by E0085/E0139.

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        // `None` literal passed where a class/type object is expected.
        // `type[X]` means a class object; passing `None` value is always wrong.
        ("type", RhsKind::NoneValue) => {
            Some("`None` (a value, not a class object — use `type(None)` or `NoneType`)")
        }
        // `type(None)` returns a class object (`NoneType`), not the value `None`.
        // A parameter annotated `None` expects the value `None`, not its type.
        ("none", RhsKind::TypeCall) => Some("`type(None)` (a class object, not the value `None`)"),
        // `type(None)` classified as a generic `CallExpr` by the resolver.
        // When the annotation is `None`, a `type(...)` call produces a class
        // object, which is incompatible with the `None` value type.
        ("none", RhsKind::CallExpr) if is_type_call(arg_source) => {
            Some("`type(None)` (a class object, not the value `None`)")
        }
        _ => None,
    }
}

/// A container parameter (`list[...]`, `set[...]`, …) that no `TypeVar`
/// assignment can satisfy: either a scalar literal argument, or a homogeneous
/// literal whose element type violates the parameter's `TypeVar` bound/constraints.
///
/// Implements the typing-spec rule that a call is an error when the collected
/// constraints for a type variable have no common solution
/// ([CHKARCH-DIAG-TYPESAFETY]).
fn container_mismatch(
    annotation: &str,
    rhs: &RhsKind,
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

    // (a) A scalar literal can never satisfy a container parameter, whatever the
    //     element type — no assignment of any TypeVar makes it valid.
    if is_scalar_literal(rhs) {
        return Some(format!(
            "a scalar literal where `{annotation}` is required — no type-variable \
             assignment makes it valid"
        ));
    }

    // (b) An invariant container of a single bounded/constrained TypeVar, given a
    //     homogeneous literal whose element type violates the bound/constraints.
    if matches!(base.as_str(), "list" | "set" | "frozenset") {
        let inner = args.first()?;
        let tv = typevars.get(inner.as_str())?;
        let elem = homogeneous_element_type(rhs)?;
        let satisfiable = match &tv.bound_type_name {
            Some(bound) => is_type_compatible(&elem, bound),
            None if !tv.constraint_type_names.is_empty() => tv
                .constraint_type_names
                .iter()
                .any(|constraint| is_type_compatible(&elem, constraint)),
            None => true,
        };
        if !satisfiable {
            return Some(format!(
                "`{base}[{elem}]` where `{annotation}` is required — `{elem}` does not \
                 satisfy type variable `{inner}`"
            ));
        }
    }
    None
}

/// `true` for a scalar literal argument (`1`, `"x"`, `True`, `b"x"`, `1.0`, `None`).
fn is_scalar_literal(rhs: &RhsKind) -> bool {
    matches!(
        rhs,
        RhsKind::IntLiteral
            | RhsKind::FloatLiteral
            | RhsKind::StrLiteral
            | RhsKind::BoolLiteral
            | RhsKind::BytesLiteral
            | RhsKind::NoneValue
    )
}

/// The element type name of a `list`/`set` literal whose elements are all the
/// same scalar literal kind (e.g. `[""]` → `str`); `None` otherwise.
fn homogeneous_element_type(rhs: &RhsKind) -> Option<String> {
    let (RhsKind::List(elements) | RhsKind::Set(elements)) = rhs else {
        return None;
    };
    let first = scalar_type_name(elements.first()?)?;
    elements
        .iter()
        .all(|elem| scalar_type_name(elem) == Some(first))
        .then(|| first.to_owned())
}

/// The Python type name for a scalar literal kind.
fn scalar_type_name(rhs: &RhsKind) -> Option<&'static str> {
    match rhs {
        RhsKind::IntLiteral => Some("int"),
        RhsKind::FloatLiteral => Some("float"),
        RhsKind::StrLiteral => Some("str"),
        RhsKind::BoolLiteral => Some("bool"),
        RhsKind::BytesLiteral => Some("bytes"),
        _ => None,
    }
}

/// Returns `true` when the argument source text is a `type(...)` call.
fn is_type_call(arg_source: Option<&str>) -> bool {
    let src = match arg_source {
        Some(s) => s.trim(),
        None => return false,
    };
    src.starts_with("type(") && src.ends_with(')')
}

fn make_diagnostic(
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
