//! Implements [`calls_argument_count`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `calls_argument_count`: Too few arguments in a function call.
//!
//! When a function is called with fewer positional arguments than it has
//! required parameters (parameters without default values), Basilisk reports
//! a missing-argument error. Handles overloaded functions by checking all
//! overload signatures.
//!
//! Also validates constructor calls: when a class is instantiated and the
//! metaclass `__call__` passes through arguments (uses `*args, **kwargs`),
//! the `__new__` or `__init__` method signature is checked for missing
//! required arguments.
//!
//! ```python
//! def func1(a: int, b: str) -> None: ...
//!
//! func1()  # E: missing required arguments
//! func1(1)  # E: missing required argument `b`
//! ```

use std::collections::HashMap;

use basilisk_resolver::{
    AttributeInfo, ClassInfo, FunctionInfo, NamedTupleDefInfo, ResolvedModule, RhsKind,
};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::shared::annotation_is_classvar;
use super::Rule;

mod method_binding;

const CODE: ErrorCode = ErrorCode {
    code: "calls_argument_count",
    docs_url: "https://www.basilisk-python.dev/errors/calls_argument_count",
};

/// Emits `calls_argument_count` for call sites with too few positional arguments.
pub(crate) struct TooFewArguments;

impl Rule for TooFewArguments {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        check_plain_function_calls(module, diagnostics);
        check_builtin_method_calls(module, diagnostics);
        method_binding::check_method_calls(module, diagnostics);
        check_constructor_calls(module, diagnostics);
        check_namedtuple_calls(module, diagnostics);
    }
}

/// Validate bound built-in method arity against the structured declarations
/// indexed from the active `builtins.pyi` generation ([STUBRES-PYI] #288).
fn check_builtin_method_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    for call in &module.calls {
        let declarations = module.builtin_methods_for_call(call);
        if declarations.is_empty() || !call.keywords.is_empty() || call.has_unpacked_kwargs {
            continue;
        }
        let provided = call.args.len();
        if declarations
            .iter()
            .any(|declaration| stub_arity_accepts(declaration, provided))
        {
            continue;
        }
        let minimum = declarations
            .iter()
            .map(|declaration| {
                declaration
                    .params
                    .iter()
                    .filter(|parameter| {
                        !parameter.has_default
                            && !matches!(
                                parameter.kind,
                                basilisk_stubs::StubParamKind::Vararg
                                    | basilisk_stubs::StubParamKind::Kwarg
                            )
                    })
                    .count()
            })
            .min()
            .unwrap_or(0);
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Call to bound built-in method `{}` has {provided} positional argument(s); no active Typeshed overload matches (minimum {minimum})",
                call.callee
            ),
            call.span,
            &module.path,
            None,
            None,
        ));
    }
}

pub(super) fn stub_arity_accepts(
    declaration: &basilisk_stubs::StubFunction,
    provided: usize,
) -> bool {
    let minimum = declaration
        .params
        .iter()
        .filter(|parameter| {
            !parameter.has_default
                && !matches!(
                    parameter.kind,
                    basilisk_stubs::StubParamKind::Vararg | basilisk_stubs::StubParamKind::Kwarg
                )
        })
        .count();
    let variadic = declaration
        .params
        .iter()
        .any(|parameter| parameter.kind == basilisk_stubs::StubParamKind::Vararg);
    let maximum = declaration
        .params
        .iter()
        .filter(|parameter| {
            !matches!(
                parameter.kind,
                basilisk_stubs::StubParamKind::Vararg | basilisk_stubs::StubParamKind::Kwarg
            )
        })
        .count();
    provided >= minimum && (variadic || provided <= maximum)
}

/// Check plain (non-constructor) function calls for too few arguments.
fn check_plain_function_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    // Group functions by name (module-level functions only).
    let mut func_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
    for func in &module.functions {
        if func.class_name.is_none() {
            func_groups
                .entry(func.name.as_str())
                .or_default()
                .push(func);
        }
    }

    for call in &module.calls {
        // Attribute calls are bound methods, not calls to a same-named
        // module-level function. Their receiver-aware declarations are checked
        // separately below; comparing `value.find(x)` with `def find(a, b)`
        // creates a false missing-argument diagnostic.
        if call.receiver.is_some() {
            continue;
        }
        let Some(funcs) = func_groups.get(call.callee.as_str()) else {
            continue;
        };

        // Skip if there are keyword arguments or `**kwargs` unpacking. Unpacked
        // kwargs (`func(**d)`) hide an unknown number of named arguments, so a
        // positional-arity check would false-positive (e.g. `func2(**td2)` where
        // `td2` supplies the required parameters). Conservative skip. [calls_argument_count]
        if !call.keywords.is_empty() || call.has_unpacked_kwargs {
            continue;
        }

        let provided_count = call.args.len();

        // Check if ANY overload matches the argument count
        let mut has_matching_overload = false;
        let mut min_required_args = usize::MAX;

        for func in funcs {
            // Skip functions with *args (they accept any number of positional args)
            if func.vararg.is_some() {
                has_matching_overload = true;
                break;
            }

            // Count required parameters (those without defaults)
            let required_count = func.parameters.iter().filter(|p| !p.has_default).count();

            min_required_args = min_required_args.min(required_count);

            // Check if this overload matches the argument count
            if provided_count >= required_count {
                has_matching_overload = true;
                break;
            }
        }

        // If no overload matches and we have overloads, emit an error
        if !has_matching_overload && funcs.len() > 1 {
            let func_name = &call.callee;
            let missing = min_required_args - provided_count;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Call to `{func_name}` is missing {missing} required argument{} \
                     (no overload matches — expected at least {min_required_args}, got {provided_count})",
                    if missing == 1 { "" } else { "s" },
                ),
                call.span,
                &module.path,
                None,
                None,
            ));
        } else if !has_matching_overload {
            // Single function case (no overloads)
            let Some(func) = funcs.first() else {
                continue;
            };
            if func.vararg.is_some() {
                continue; // *args accepts any number
            }

            let required_count = func.parameters.iter().filter(|p| !p.has_default).count();
            if provided_count < required_count {
                let missing = required_count - provided_count;
                let func_name = &func.name;
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Call to `{func_name}` is missing {missing} required argument{} \
                         (expected {required_count}, got {provided_count})",
                        if missing == 1 { "" } else { "s" },
                    ),
                    call.span,
                    &module.path,
                    None,
                    None,
                ));
            }
        }
    }
}

/// Returns `true` when the metaclass `__call__` method passes through arguments
/// to the underlying `__new__`/`__init__` (i.e. its `__call__` uses `*args, **kwargs`
/// and does not have a fixed return type that overrides the constructor).
///
/// When the metaclass `__call__` has a concrete, non-generic return type that is
/// NOT the class being constructed (e.g. `-> NoReturn`, `-> int | Meta`), the
/// metaclass fully controls the constructor call and we should NOT validate
/// arguments against `__new__`/`__init__`.
fn metaclass_passes_through(metaclass_name: &str, module: &ResolvedModule) -> bool {
    // First check that the metaclass class exists
    if !module.classes.iter().any(|c| c.name == metaclass_name) {
        return false;
    }

    // Find the metaclass __call__ method
    let call_method = module
        .functions
        .iter()
        .find(|f| f.class_name.as_deref() == Some(metaclass_name) && f.name == "__call__");

    let Some(call_fn) = call_method else {
        // No __call__ method on metaclass — default type.__call__ passes through
        return true;
    };

    // The metaclass __call__ must use *args and **kwargs to pass through
    call_fn.vararg.is_some() && call_fn.kwarg.is_some() && constructs_an_instance(call_fn, module)
}

/// Does this metaclass `__call__` still yield an instance of the class being
/// constructed, so `__new__`/`__init__` are evaluated as usual?
///
/// Per the [metaclass `__call__`](https://typing.python.org/en/latest/spec/constructors.html#metaclass-call-method)
/// rules, a return annotated with a type variable
/// (`def __call__(cls: type[T], ...) -> T`) or `Self` is the pass-through
/// spelling. Any other concrete return — `NoReturn`, `int | Meta` — means the
/// metaclass fully controls the call and the constructor signature is never
/// consulted, so an arity judgment against `__new__` would be a false positive.
///
/// An UNANNOTATED `__call__` is decided from its body instead of assumed, so
/// this judgment survives [TYPEINF-TARGET-GRADUAL]: stripping the annotations
/// off a metaclass must not turn a silent constructor call into an error.
fn constructs_an_instance(call_fn: &FunctionInfo, module: &ResolvedModule) -> bool {
    let Some(span) = call_fn.return_annotation_span else {
        return body_delegates_construction(call_fn);
    };
    let Some(text) = slice_span(&module.source, span) else {
        return body_delegates_construction(call_fn);
    };
    let returned = text.trim();
    returned == "Self"
        || module
            .typevar_calls
            .iter()
            .any(|typevar| typevar.name == returned)
}

/// Does an unannotated metaclass `__call__` hand construction back to the
/// normal machinery?
///
/// `return type.__call__(cls, *args, **kwargs)` delegates, so `__new__` runs and
/// its signature governs. A body that returns a value of its own (`return 1`) or
/// never returns at all (`raise TypeError(...)`) produces something that is not
/// an instance of the class, so the constructor is never consulted.
fn body_delegates_construction(call_fn: &FunctionInfo) -> bool {
    let mut returns_values = call_fn.return_stmts.iter().filter(|stmt| stmt.has_value);
    returns_values.clone().next().is_some() && returns_values.all(|stmt| stmt.value_is_call)
}

/// Collects the positional (non-kw_only, non-init_false, non-ClassVar) fields of a
/// dataclass in declaration order. These are the fields that correspond positionally
/// to constructor arguments when no keyword arguments are used.
fn positional_dataclass_fields<'a>(
    class_info: &'a ClassInfo,
    resolver: &crate::annotation::AnnotationResolver<'_>,
    source: &str,
) -> Vec<&'a AttributeInfo> {
    class_info
        .attributes
        .iter()
        .filter(|a| {
            a.has_annotation
                && !a.is_init_false
                && !a.is_kw_only
                && !a.is_init_var
                && !annotation_is_classvar(resolver, source, a.annotation_span)
        })
        .collect()
}

/// Counts the required (no-default) positional dataclass fields.
fn required_dataclass_field_count(
    class_info: &ClassInfo,
    resolver: &crate::annotation::AnnotationResolver<'_>,
    source: &str,
) -> usize {
    positional_dataclass_fields(class_info, resolver, source)
        .into_iter()
        .filter(|a| !a.has_value)
        .count()
}

/// Maps a [`RhsKind`] literal to its Python type name, or `None` for non-literals.
fn rhs_kind_to_type_name(kind: &RhsKind) -> Option<&'static str> {
    match kind {
        RhsKind::IntLiteral => Some("int"),
        RhsKind::FloatLiteral => Some("float"),
        RhsKind::StrLiteral => Some("str"),
        RhsKind::BoolLiteral => Some("bool"),
        RhsKind::BytesLiteral => Some("bytes"),
        _ => None,
    }
}

/// Returns `true` when passing a value of `arg_type` to a parameter annotated
/// `param_ann` is clearly a type mismatch.
///
/// Only flags clear incompatibilities between primitive literal types to avoid
/// false positives on complex or unknown types.
fn is_clearly_incompatible(arg_type: &str, param_ann: &str) -> bool {
    let param = param_ann.trim();
    // Remove optional suffix / union — only check the primary type name.
    // e.g. "int | None" → still allow "int" args; "str" args would be wrong.
    let primary = param.split('|').next().unwrap_or(param).trim();
    match arg_type {
        "str" => matches!(primary, "int" | "float" | "bool" | "bytes"),
        "bytes" => matches!(primary, "int" | "float" | "bool" | "str"),
        "int" => matches!(primary, "str" | "bytes"),
        "float" => matches!(primary, "str" | "bytes"),
        _ => false,
    }
}

/// Check positional argument types against the dataclass field types.
///
/// Only emits errors when a literal argument type is clearly incompatible
/// with the corresponding field annotation.
///
/// Skips classes with dataclass bases because Python dataclass inheritance
/// reorders fields according to the MRO, which may differ from the
/// declaration order we see in the class body.
fn check_dataclass_arg_types(
    class_info: &ClassInfo,
    call: &basilisk_resolver::CallSite,
    resolver: &crate::annotation::AnnotationResolver<'_>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Dataclass inheritance reorders fields based on MRO.  Without full MRO
    // resolution we cannot map positional arguments to the correct fields, so
    // bail out to avoid false positives.
    if !class_info.bases.is_empty() {
        return;
    }

    let fields = positional_dataclass_fields(class_info, resolver, source);

    for (idx, (arg_kind, arg_span)) in call.args.iter().enumerate() {
        let Some(field) = fields.get(idx) else {
            break;
        };
        let Some(arg_type) = rhs_kind_to_type_name(arg_kind) else {
            continue;
        };
        let Some(ann_span) = field.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        if is_clearly_incompatible(arg_type, ann_text) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument {} to `{}()` has type `{arg_type}` but field `{}` expects `{}`",
                    idx + 1,
                    class_info.name,
                    field.name,
                    ann_text.trim(),
                ),
                *arg_span,
                path,
                None,
                None,
            ));
        }
    }
}

/// Check constructor calls (class instantiation) for too few arguments.
///
/// When a class is called as a constructor, we validate arguments against the
/// class's `__new__` or `__init__` method, but only if the metaclass `__call__`
/// passes arguments through (uses `*args, **kwargs`).
fn check_constructor_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let Some(resolver) = crate::annotation::AnnotationResolver::for_module(module) else {
        return;
    };
    // Build a map of class names for quick lookup.
    let class_map = super::shared::class_name_map(&module.classes);

    // Build a map of (class_name, method_name) → FunctionInfo for methods.
    let method_map = super::shared::method_name_map(&module.functions);

    for call in &module.calls {
        // Only process constructor calls (callee matches a class name)
        let Some(class_info) = class_map.get(call.callee.as_str()) else {
            continue;
        };

        // Skip if there are keyword arguments or `**kwargs` unpacking. Unpacked
        // kwargs (`func(**d)`) hide an unknown number of named arguments, so a
        // positional-arity check would false-positive (e.g. `func2(**td2)` where
        // `td2` supplies the required parameters). Conservative skip. [calls_argument_count]
        if !call.keywords.is_empty() || call.has_unpacked_kwargs {
            continue;
        }

        // Check metaclass: if the class has a metaclass that does NOT pass through
        // arguments, skip validation (the metaclass controls the call signature).
        if let Some(ref meta_name) = class_info.metaclass_name {
            if !metaclass_passes_through(meta_name, module) {
                continue;
            }
        }

        // Find the constructor method to validate against.
        // Priority: __new__ first, then __init__.
        let constructor_method = find_constructor_method(class_info, &method_map);

        if let Some(constructor) = constructor_method {
            // For constructor methods, the first parameter (cls/self) is implicit.
            // Skip it when counting required parameters.
            let required_count = constructor
                .parameters
                .iter()
                .skip(1) // skip cls/self
                .filter(|p| !p.has_default)
                .count();

            // Constructor with *args accepts any number of positional args
            if constructor.vararg.is_some() {
                continue;
            }

            let provided_count = call.args.len();
            if provided_count < required_count {
                let missing = required_count - provided_count;
                let class_name = &class_info.name;
                let method_name = &constructor.name;
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Call to `{class_name}()` is missing {missing} required argument{} \
                         for `{method_name}` (expected {required_count}, got {provided_count})",
                        if missing == 1 { "" } else { "s" },
                    ),
                    call.span,
                    &module.path,
                    None,
                    None,
                ));
            }
        } else if class_info.is_dataclass {
            check_dataclass_no_explicit_constructor(
                class_info,
                call,
                &resolver,
                &module.source,
                &module.path,
                diagnostics,
            );
        }
    }
}

/// Validate a call to a dataclass that has no explicit `__new__` or `__init__`.
///
/// Two sub-cases:
/// 1. `@dataclass(init=False)` with no custom `__init__`: any positional
///    arguments are an error (the class falls back to `object.__init__` which
///    accepts no positional args).
/// 2. Normal dataclass (synthesised `__init__`): check that the caller
///    provides at least as many positional arguments as there are required
///    fields, and that each literal argument is compatible with the
///    corresponding field type.
fn check_dataclass_no_explicit_constructor(
    class_info: &ClassInfo,
    call: &basilisk_resolver::CallSite,
    resolver: &crate::annotation::AnnotationResolver<'_>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let provided_count = call.args.len();

    if class_info.is_dataclass_init_false {
        // init=False with no custom __init__: object.__init__ accepts 0 args.
        if provided_count > 0 {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot pass arguments to `{}()`: `@dataclass(init=False)` is set \
                     with no explicit `__init__` method defined",
                    class_info.name
                ),
                call.span,
                path,
                Some(
                    "Define an explicit `__init__` method or remove the `init=False` flag"
                        .to_owned(),
                ),
                None,
            ));
        }
        return;
    }

    // Normal synthesised __init__: check required field count.
    let required_count = required_dataclass_field_count(class_info, resolver, source);

    if provided_count < required_count {
        let missing = required_count - provided_count;
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Call to `{}()` is missing {missing} required field argument{} \
                 (expected at least {required_count}, got {provided_count})",
                class_info.name,
                if missing == 1 { "" } else { "s" },
            ),
            call.span,
            path,
            None,
            None,
        ));
    }

    // Also check argument types for literal arguments.
    if provided_count > 0 {
        check_dataclass_arg_types(class_info, call, resolver, source, path, diagnostics);
    }
}

/// Find the constructor method (`__new__` or `__init__`) to validate against.
///
/// Returns the `__new__` method if it exists and has a non-trivial signature
/// (not just `*args, **kwargs`), otherwise falls back to `__init__`.
fn find_constructor_method<'a>(
    class_info: &ClassInfo,
    method_map: &HashMap<(&str, &str), Vec<&'a FunctionInfo>>,
) -> Option<&'a FunctionInfo> {
    let class_name = class_info.name.as_str();

    // Try __new__ first
    if let Some(new_methods) = method_map.get(&(class_name, "__new__")) {
        // Use the first non-overload __new__, or the first one
        let new_fn = new_methods
            .iter()
            .find(|f| !super::shared::decorator_spelled(&f.decorators, "overload"))
            .or_else(|| new_methods.first());
        if let Some(func) = new_fn {
            return Some(func);
        }
    }

    // Fall back to __init__
    if let Some(init_methods) = method_map.get(&(class_name, "__init__")) {
        let init_fn = init_methods
            .iter()
            .find(|f| !super::shared::decorator_spelled(&f.decorators, "overload"))
            .or_else(|| init_methods.first());
        if let Some(func) = init_fn {
            return Some(func);
        }
    }

    None
}

/// Check calls to functional-form `NamedTuple` / `namedtuple` for argument count errors.
///
/// Validates:
/// - Too few positional args (below required field count, considering defaults)
/// - Too many positional args (above total field count)
fn check_namedtuple_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let nt_map: HashMap<&str, &NamedTupleDefInfo> = module
        .namedtuple_defs
        .iter()
        .map(|nt| (nt.lhs_name.as_str(), nt))
        .collect();

    for call in &module.calls {
        let Some(nt) = nt_map.get(call.callee.as_str()) else {
            continue;
        };

        let total_fields = nt.field_names.len();
        let required_fields = total_fields.saturating_sub(nt.defaults_count);
        let positional_count = call.args.len();
        // Keywords that are valid field names count toward satisfying field requirements.
        let keyword_field_count = call
            .keywords
            .iter()
            .filter(|(kw, _)| nt.field_names.iter().any(|f| f == kw))
            .count();
        let total_provided = positional_count + keyword_field_count;

        if total_provided < required_fields {
            let missing = required_fields - total_provided;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Call to `{}()` is missing {missing} required argument{} \
                     (expected at least {required_fields}, got {positional_count})",
                    nt.lhs_name,
                    if missing == 1 { "" } else { "s" },
                ),
                call.span,
                &module.path,
                None,
                None,
            ));
        } else if positional_count > total_fields {
            let extra = positional_count - total_fields;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Call to `{}()` has {extra} too many positional argument{} \
                     (expected at most {total_fields}, got {positional_count})",
                    nt.lhs_name,
                    if extra == 1 { "" } else { "s" },
                ),
                call.span,
                &module.path,
                None,
                None,
            ));
        }
    }
}
