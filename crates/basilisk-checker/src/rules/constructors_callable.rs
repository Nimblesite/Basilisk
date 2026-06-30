//! Implements [`constructors_callable`] from [CHKARCH-DIAG-CTOR-CALLABLE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ctor-callable
//! `constructors_callable`: Invalid call to a constructor-derived callable.
//!
//! Implements the typing spec rule "Converting a constructor to callable"
//! (<https://typing.readthedocs.io/en/latest/spec/constructors.html#converting-a-constructor-to-callable>).
//!
//! When a class object flows through an identity-over-callable function such as
//!
//! ```python
//! def accepts_callable(cb: Callable[P, R]) -> Callable[P, R]:
//!     return cb
//!
//! r1 = accepts_callable(Class1)   # r1 has Class1's constructor signature
//! ```
//!
//! the bound variable (`r1`) gains the *constructor-to-callable* signature of
//! the class. Calls to that variable must match the synthesized signature:
//!
//! ```python
//! r1()      # E0153: missing required argument `x`
//! r1(y=1)   # E0153: unexpected keyword argument `y`
//! ```
//!
//! The synthesized signature is derived (in priority order) from the
//! metaclass `__call__`, then `__new__` (when it returns a type other than the
//! class / `Self`), then `__init__`, mirroring runtime construction.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ParameterInfo, ResolvedModule, Span};
use ruff_python_ast::{Expr, ExprCall, Number, Stmt};
use ruff_text_size::Ranged as _;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "constructors_callable",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_callable",
};

/// Method-map key: `(class_name, method_name)` → method definitions.
type MethodMap<'a> = HashMap<(&'a str, &'a str), Vec<&'a FunctionInfo>>;

/// Emits `constructors_callable` for invalid calls to constructor-derived callables.
pub(crate) struct ConstructorCallableMisuse;

impl Rule for ConstructorCallableMisuse {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), module.path.clone()) else {
            return;
        };

        let class_map = super::shared::class_name_map(&module.classes);
        let method_map = super::shared::method_name_map(&module.functions);
        let identity_fns = collect_identity_callables(&module.functions, source);
        let var_to_class = map_vars_to_classes(&parsed.ast.body, &identity_fns, &class_map);
        if var_to_class.is_empty() {
            return;
        }
        let typevars = basilisk_resolver::collect_names(&module.typevar_calls);

        basilisk_resolver::visit_calls(&parsed.ast.body, &mut |call| {
            let Expr::Name(callee) = call.func.as_ref() else {
                return;
            };
            let Some(class_name) = var_to_class.get(callee.id.as_str()) else {
                return;
            };
            let sig = build_converted_callable(class_name, &class_map, &method_map, source);
            validate_call(call, class_name, &sig, &typevars, &module.path, diagnostics);
        });
    }
}

/// The constructor-to-callable signature synthesized for a class.
struct ConvertedCallable<'a> {
    /// Parameters after dropping the implicit `cls`/`self`.
    params: Vec<&'a ParameterInfo>,
    /// `true` when the controlling method accepts `*args`.
    has_var_positional: bool,
    /// `true` when the controlling method accepts `**kwargs`.
    has_var_keyword: bool,
}

/// Collect module-level identity-over-callable functions.
///
/// A function `f` is an identity callable when it has a single parameter
/// annotated `Callable[...]` and an identical return annotation, e.g.
/// `def f(cb: Callable[P, R]) -> Callable[P, R]`. Calling such a function with
/// a class argument yields a value with that class's constructor signature.
fn collect_identity_callables<'a>(functions: &'a [FunctionInfo], source: &str) -> Vec<&'a str> {
    functions
        .iter()
        .filter(|f| is_identity_callable(f, source))
        .map(|f| f.name.as_str())
        .collect()
}

/// Returns `true` when `func` is a single-argument identity-over-callable.
fn is_identity_callable(func: &FunctionInfo, source: &str) -> bool {
    if func.class_name.is_some()
        || func.vararg.is_some()
        || func.kwarg.is_some()
        || func.parameters.len() != 1
    {
        return false;
    }
    let Some(param_ann) = func
        .parameters
        .first()
        .and_then(|p| p.annotation_text.as_deref())
    else {
        return false;
    };
    let Some(return_text) = func
        .return_annotation_span
        .and_then(|span| span.slice_source(source))
    else {
        return false;
    };
    let param = param_ann.trim();
    param == return_text.trim() && is_callable_type_text(param)
}

/// Returns `true` when `text` denotes a `Callable[...]` type expression.
fn is_callable_type_text(text: &str) -> bool {
    text.starts_with("Callable[") || text.contains(".Callable[")
}

/// Build the map from a bound variable to the class whose constructor it wraps.
///
/// Only top-level `var = identity_fn(KnownClass)` assignments are recognized.
fn map_vars_to_classes<'a>(
    body: &[Stmt],
    identity_fns: &[&str],
    class_map: &HashMap<&'a str, &'a ClassInfo>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for stmt in body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        let [Expr::Name(target)] = assign.targets.as_slice() else {
            continue;
        };
        if let Some(class) = wrapped_class(assign.value.as_ref(), identity_fns, class_map) {
            let _ = map.insert(target.id.to_string(), class.to_owned());
        }
    }
    map
}

/// Extract the class name from `identity_fn(KnownClass)`, if the RHS matches.
fn wrapped_class<'a>(
    value: &Expr,
    identity_fns: &[&str],
    class_map: &HashMap<&'a str, &'a ClassInfo>,
) -> Option<&'a str> {
    let Expr::Call(call) = value else {
        return None;
    };
    let Expr::Name(callee) = call.func.as_ref() else {
        return None;
    };
    if !identity_fns.contains(&callee.id.as_str()) || !call.arguments.keywords.is_empty() {
        return None;
    }
    let [Expr::Name(arg)] = call.arguments.args.as_ref() else {
        return None;
    };
    class_map
        .get_key_value(arg.id.as_str())
        .map(|(name, _)| *name)
}

/// Synthesize the constructor-to-callable signature for `class_name`.
///
/// Priority: a custom metaclass `__call__`, then `__new__` (when its return
/// type is neither `Self` nor the class), then `__init__`. A class with no
/// controlling method synthesizes a zero-argument callable.
fn build_converted_callable<'a>(
    class_name: &str,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    method_map: &MethodMap<'a>,
    source: &str,
) -> ConvertedCallable<'a> {
    if let Some(call_fn) = metaclass_call(class_name, class_map, method_map) {
        return from_method(call_fn);
    }

    let new_impl = method_map
        .get(&(class_name, "__new__"))
        .and_then(|v| pick_impl(v));
    let init_impl = method_map
        .get(&(class_name, "__init__"))
        .and_then(|v| pick_impl(v));

    let controlling = match (new_impl, init_impl) {
        (Some(new_fn), _) if new_controls(new_fn, class_name, source) => Some(new_fn),
        (_, Some(init_fn)) => Some(init_fn),
        (Some(new_fn), None) => Some(new_fn),
        (None, None) => None,
    };

    controlling.map_or_else(
        || ConvertedCallable {
            params: Vec::new(),
            has_var_positional: false,
            has_var_keyword: false,
        },
        from_method,
    )
}

/// Return the custom metaclass `__call__` controlling construction, if any.
///
/// A class whose declared metaclass defines `__call__` has that method govern
/// the call signature (e.g. a `__call__` with `*args, **kwargs` accepts any
/// arguments). Classes without an explicit metaclass — or whose metaclass only
/// inherits `type.__call__` — fall through to `__new__`/`__init__`.
fn metaclass_call<'a>(
    class_name: &str,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    method_map: &MethodMap<'a>,
) -> Option<&'a FunctionInfo> {
    let metaclass = class_map.get(class_name)?.metaclass_name.as_deref()?;
    pick_impl(method_map.get(&(metaclass, "__call__"))?)
}

/// Build a converted signature from a controlling method (drops `cls`/`self`).
fn from_method(method: &FunctionInfo) -> ConvertedCallable<'_> {
    ConvertedCallable {
        params: method.parameters.iter().skip(1).collect(),
        has_var_positional: method.vararg.is_some(),
        has_var_keyword: method.kwarg.is_some(),
    }
}

/// Pick the non-`@overload` implementation, falling back to the first entry.
fn pick_impl<'a>(funcs: &[&'a FunctionInfo]) -> Option<&'a FunctionInfo> {
    funcs
        .iter()
        .find(|f| !f.decorators.iter().any(|d| d == "overload"))
        .or_else(|| funcs.first())
        .copied()
}

/// Returns `true` when `__new__`'s return type takes over construction.
///
/// Per the spec, when `__new__` returns a type that is neither `Self` nor the
/// class itself, `__init__` is ignored and the converted callable uses
/// `__new__`'s signature and return type.
fn new_controls(new_fn: &FunctionInfo, class_name: &str, source: &str) -> bool {
    let Some(text) = new_fn
        .return_annotation_span
        .and_then(|span| span.slice_source(source))
    else {
        return false;
    };
    let text = text.trim();
    if last_segment(text) == "Self" {
        return false;
    }
    let head = text.split('[').next().unwrap_or(text).trim();
    last_segment(head) != class_name
}

/// Return the final `.`-separated segment of `s` (`typing.Self` → `Self`).
fn last_segment(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or(s)
}

/// Validate a call against the synthesized signature, emitting one diagnostic.
fn validate_call(
    call: &ExprCall,
    class_name: &str,
    sig: &ConvertedCallable<'_>,
    typevars: &[&str],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Conservative: starred positional / `**kwargs` unpacking defeats arity
    // analysis, so skip rather than risk a false positive.
    if call
        .arguments
        .args
        .iter()
        .any(|a| matches!(a, Expr::Starred(_)))
        || call.arguments.keywords.iter().any(|k| k.arg.is_none())
    {
        return;
    }

    let positional = call.arguments.args.len();
    let kw_names: Vec<&str> = call
        .arguments
        .keywords
        .iter()
        .filter_map(|k| k.arg.as_ref().map(ruff_python_ast::Identifier::as_str))
        .collect();

    if let Some(diag) = check_keywords(call, class_name, sig, &kw_names, path) {
        diagnostics.push(diag);
    } else if let Some(diag) = check_too_many(call, class_name, sig, positional, path) {
        diagnostics.push(diag);
    } else if let Some(diag) = check_missing(call, class_name, sig, positional, &kw_names, path) {
        diagnostics.push(diag);
    } else if let Some(diag) = check_typevar_conflict(call, class_name, sig, typevars, path) {
        diagnostics.push(diag);
    }
}

/// Flag the first keyword that names no parameter (when no `**kwargs`).
fn check_keywords(
    call: &ExprCall,
    class_name: &str,
    sig: &ConvertedCallable<'_>,
    kw_names: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    if sig.has_var_keyword {
        return None;
    }
    let unknown = kw_names
        .iter()
        .find(|name| !sig.params.iter().any(|p| p.name == **name))?;
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Unexpected keyword argument `{unknown}` in call to the constructor-derived \
             callable for `{class_name}`"
        ),
        Span::from(call.range()),
        path,
        Some("Remove the keyword argument or pass it positionally".to_owned()),
        None,
    ))
}

/// Flag too many positional arguments (when no `*args`).
fn check_too_many(
    call: &ExprCall,
    class_name: &str,
    sig: &ConvertedCallable<'_>,
    positional: usize,
    path: &str,
) -> Option<Diagnostic> {
    if sig.has_var_positional || positional <= sig.params.len() {
        return None;
    }
    let max = sig.params.len();
    let extra = positional - max;
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Call to the constructor-derived callable for `{class_name}` has {extra} too many \
             positional argument{} (expected at most {max}, got {positional})",
            if extra == 1 { "" } else { "s" },
        ),
        Span::from(call.range()),
        path,
        None,
        None,
    ))
}

/// Flag the first required parameter left unsatisfied by position or keyword.
fn check_missing(
    call: &ExprCall,
    class_name: &str,
    sig: &ConvertedCallable<'_>,
    positional: usize,
    kw_names: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    let missing = sig.params.iter().enumerate().find(|(idx, param)| {
        !param.has_default && *idx >= positional && !kw_names.contains(&param.name.as_str())
    })?;
    let required = sig.params.iter().filter(|p| !p.has_default).count();
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Call to the constructor-derived callable for `{class_name}` is missing required \
             argument `{}` (expected {required}, got {positional})",
            missing.1.name,
        ),
        Span::from(call.range()),
        path,
        None,
        None,
    ))
}

/// Flag a `TypeVar` bound to two incompatible types across `list[T]` params.
fn check_typevar_conflict(
    call: &ExprCall,
    class_name: &str,
    sig: &ConvertedCallable<'_>,
    typevars: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    let mut bindings: HashMap<&str, &'static str> = HashMap::new();
    for (idx, param) in sig.params.iter().enumerate() {
        let Some(arg) = call.arguments.args.get(idx) else {
            break;
        };
        let Some(tv) = list_typevar(param.annotation_text.as_deref(), typevars) else {
            continue;
        };
        let Some(element) = list_literal_element(arg) else {
            // A non-list scalar literal can never satisfy a `list[tv]` parameter:
            // no assignment of `tv` makes it valid.
            if let Some(scalar) = non_list_scalar_literal(arg) {
                return Some(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Argument for the `list[{tv}]` parameter of the constructor-derived \
                         callable for `{class_name}` is a `{scalar}` literal — no assignment \
                         of type variable `{tv}` makes it valid"
                    ),
                    Span::from(arg.range()),
                    path,
                    None,
                    None,
                ));
            }
            continue;
        };
        if let Some(previous) = bindings.insert(tv, element) {
            if previous != element {
                return Some(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Inconsistent binding for type variable `{tv}` in call to the \
                         constructor-derived callable for `{class_name}`: `list[{previous}]` \
                         vs `list[{element}]`"
                    ),
                    Span::from(call.range()),
                    path,
                    None,
                    None,
                ));
            }
        }
    }
    None
}

/// If `annotation` is `list[T]` where `T` is a known `TypeVar`, return `T`.
fn list_typevar<'a>(annotation: Option<&'a str>, typevars: &[&str]) -> Option<&'a str> {
    let text = annotation?.trim();
    let inner = text.strip_prefix("list[")?.strip_suffix(']')?.trim();
    typevars.contains(&inner).then_some(inner)
}

/// A scalar literal that is definitely not a list (so it can never satisfy a
/// `list[...]` parameter).
fn non_list_scalar_literal(arg: &Expr) -> Option<&'static str> {
    match arg {
        Expr::List(_) => None,
        _ => literal_type_name(arg),
    }
}

/// Return the literal element type of a homogeneous list literal `[lit, ...]`.
fn list_literal_element(arg: &Expr) -> Option<&'static str> {
    let Expr::List(list) = arg else {
        return None;
    };
    let first = literal_type_name(list.elts.first()?)?;
    list.elts
        .iter()
        .all(|elt| literal_type_name(elt) == Some(first))
        .then_some(first)
}

/// Map a literal expression to its Python type name, or `None` for non-literals.
fn literal_type_name(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::StringLiteral(_) | Expr::FString(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NumberLiteral(n) => match n.value {
            Number::Int(_) => Some("int"),
            Number::Float(_) => Some("float"),
            Number::Complex { .. } => Some("complex"),
        },
        _ => None,
    }
}
