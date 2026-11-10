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

use basilisk_resolver::{AttributeInfo, ClassInfo, FunctionInfo, ResolvedModule, RhsKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

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
fn check_plain_function_calls(_module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
    // DELETED: this grouped definitions by `FunctionInfo::name` and joined
    // calls through `CallSite::callee`, both renderings. Aliases, rebindings,
    // and same-spelled definitions therefore changed the arity verdict.
    panic!(
        "basilisk-checker: `check_plain_function_calls` was DELETED because it joined \
         calls to functions by rendered callee/definition names. The resolver must carry \
         the callee's function definition site at the call offset. Do not restore the \
         name map or silently skip the rule."
    )
    /*
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
    */
}

// ##########################################################################
// # DELETED BODY — `metaclass_passes_through`. DO NOT RESTORE IT.
// #
// #   module.classes.iter().any(|c| c.name == metaclass_name)
// #   f.class_name.as_deref() == Some(metaclass_name)
// #
// # `ClassInfo::metaclass_name` is the RENDERED text of a `metaclass=` value,
// # matched against class names and method owners by string. An imported
// # metaclass, one reached through an alias, or `metaclass=mod.Meta` all
// # failed to resolve, and a local class sharing the rendered name matched.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn metaclass_passes_through(
    graph: &basilisk_resolver::ClassGraph<'_>,
    method_map: &std::collections::HashMap<(basilisk_resolver::Span, &str), Vec<&FunctionInfo>>,
    class_info: &ClassInfo,
    module: &ResolvedModule,
) -> bool {
    // No `metaclass=` naming a class of this module: nothing here can
    // intercept construction, so the ordinary constructor check applies.
    let Some(meta) = class_info.metaclass_site.and_then(|site| graph.at(site)) else {
        return true;
    };
    // `__call__` may be declared by the metaclass or inherited by it.
    let Some(call_fn) = graph
        .ancestors(meta)
        .into_iter()
        .find_map(|ancestor| method_map.get(&(ancestor.name_span, "__call__")))
        .and_then(|defs| defs.last().copied())
    else {
        return true;
    };
    constructs_an_instance(call_fn, module)
}

/// Does this metaclass `__call__` still yield an instance of the class being
/// constructed, so `__new__`/`__init__` are evaluated as usual?
///
/// Per the [metaclass `__call__`](https://typing.python.org/en/latest/spec/constructors.html#metaclass-call-method)
/// rules, a return annotated with a type variable
/// (`def __call__(cls: type[T], ...) -> T`) is the pass-through form. Any other
/// concrete return means the metaclass fully controls the call and the
/// constructor signature is never consulted, so an arity judgment against
/// `__new__` would be a false positive.
///
/// An UNANNOTATED `__call__` is decided from its body instead of assumed, so
/// this judgment survives [TYPEINF-TARGET-GRADUAL]: stripping the annotations
/// off a metaclass must not turn a silent constructor call into an error.
///
// ##########################################################################
// # DELETED BODY — `constructs_an_instance`. DO NOT RESTORE IT.            #
// #                                                                         #
// #   let text = slice_span(&module.source, span)?;                        #
// #   module.typevar_calls.iter().any(|tv| tv.name == text.trim())         #
// #                                                                         #
// # The return annotation was SLICED OUT OF THE FILE and its trimmed        #
// # characters compared against the NAME a `TypeVar` was declared with.     #
// # This decides whether a metaclass `__call__` passes construction through #
// # to `__new__`/`__init__`, so it gates every constructor arity            #
// # diagnostic on the class:                                                #
// #                                                                         #
// #   * `-> T` and `-> "T"` are the same annotation and compare unequal;    #
// #   * `Alias = T` used as `-> Alias` denotes the SAME `TypeVar` object    #
// #     and does not match, silently dropping the diagnostic;               #
// #   * a class or variable merely SPELLED like a `TypeVar` matches;        #
// #   * a qualified spelling, or whitespace the slice happens to include,   #
// #     changes the answer.                                                 #
// #                                                                         #
// # The lawful replacement resolves the return annotation's `Expr` through  #
// # the binding table and compares the resulting definition site against    #
// # the `TypeVar` construction's own `TypeVarCallInfo::span` — the same     #
// # mechanism `generics_basic_2` uses. `FunctionInfo` carries only a SPAN   #
// # for the annotation, so the caller must first reach the node through     #
// # `rules::shared::ExprIndex`.                                             #
// #                                                                         #
// # Pinned by: tests/constructor_identity_tests.rs                          #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn constructs_an_instance(_call_fn: &FunctionInfo, _module: &ResolvedModule) -> bool {
    panic!(
        "basilisk-checker: `constructs_an_instance` was DELETED because it decided \
         whether a metaclass `__call__` passes construction through by SLICING the \
         return annotation out of the source and comparing its characters to a \
         `TypeVar`'s declared name. It panics because the real implementation — the \
         annotation's `Expr` resolved through the binding table and compared against \
         `TypeVarCallInfo::span` — DOES NOT EXIST YET. Do not restore the slice and do \
         not return `true` or `false` in its place."
    )
}

/// Does an unannotated metaclass `__call__` hand construction back to the
/// normal machinery?
///
/// `return type.__call__(cls, *args, **kwargs)` delegates, so `__new__` runs and
/// its signature governs. A body that returns a value of its own (`return 1`) or
/// never returns at all (`raise TypeError(...)`) produces something that is not
/// an instance of the class, so the constructor is never consulted.
///
/// ORPHANED, NOT DELETED. It reads `FunctionInfo::return_stmts` — resolver
/// structure, no text — and does not have `constructs_an_instance`'s defect.
/// The rebuild calls it unchanged for the unannotated case.
#[expect(
    dead_code,
    reason = "caller deleted for slicing the return annotation; this structural helper \
              is retained for the rebuild — see the `constructs_an_instance` banner"
)]
fn body_delegates_construction(call_fn: &FunctionInfo) -> bool {
    let mut returns_values = call_fn.return_stmts.iter().filter(|stmt| stmt.has_value);
    returns_values.clone().next().is_some() && returns_values.all(|stmt| stmt.value_is_call)
}

/// Collects the positional (non-kw_only, non-init_false, non-ClassVar) fields of a
/// dataclass in declaration order. These are the fields that correspond positionally
/// to constructor arguments when no keyword arguments are used.
fn positional_dataclass_fields(class_info: &ClassInfo) -> Vec<&AttributeInfo> {
    class_info
        .attributes
        .iter()
        .filter(|a| {
            a.has_annotation
                && !a.is_init_false
                && !a.is_kw_only
                && !a.is_init_var
                && !a.is_class_var
        })
        .collect()
}

/// Counts the required (no-default) positional dataclass fields.
fn required_dataclass_field_count(class_info: &ClassInfo) -> usize {
    positional_dataclass_fields(class_info)
        .into_iter()
        .filter(|a| !a.has_value)
        .count()
}

// ##########################################################################
// # DELETED BODY — `rhs_kind_to_type_name`. DO NOT RESTORE IT.            #
// #                                                                        #
// #   RhsKind::IntLiteral => Some("int"), …                                #
// #                                                                        #
// # This is the BRIDGE that turned a fact the parser knew — the kind of    #
// # literal node — back into a SPELLING, so that the comparison below      #
// # could be done on characters. `RhsKind::IntLiteral` already says        #
// # everything `"int"` says and cannot be confused with a user class of    #
// # that name; rendering it discards exactly the identity that matters.    #
// #                                                                        #
// # Every deletion in this crate ends up here: some helper renders a       #
// # resolved fact to text so that a `match` on string literals becomes     #
// # possible. The rendering is the defect, not the `match`.                #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn rhs_kind_to_type_name(_kind: &RhsKind) -> Option<&'static str> {
    panic!(
        "basilisk-checker: `rhs_kind_to_type_name` was DELETED because it RENDERED a \
         known literal kind back into a type SPELLING so the comparison downstream \
         could be done on characters. It panics because the real implementation — \
         carrying the literal's canonical `TypeNode` to the comparison instead of its \
         name — DOES NOT EXIST YET. Do not restore the rendering and do not return \
         `None` in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `is_clearly_incompatible`. DO NOT RESTORE IT AND DO NOT #
// # RETURN `false` IN ITS PLACE.                                           #
// #                                                                        #
// # `fn name_subtype` by another name — the exact construct the spelling   #
// # guard forbids. It settled ASSIGNABILITY between two RENDERED           #
// # SPELLINGS:                                                             #
// #                                                                        #
// #   let primary = param.split('|').next().unwrap_or(param).trim();       #
// #   match arg_type {                                                     #
// #       "str"   => matches!(primary, "int" | "float" | "bool" | "bytes"),#
// #       "int"   => matches!(primary, "str" | "bytes"), …                 #
// #   }                                                                    #
// #                                                                        #
// # Splitting on the `|` CHARACTER is not union decomposition: it cuts     #
// # `Callable[[int | str], None]` at the wrong `|`, cuts inside a          #
// # `Literal[\"a|b\"]`, and does not see `Optional[int]` or `Union[int,    #
// # str]` as unions at all. The type names are builtin identity by         #
// # spelling, so an aliased import was never incompatible and a module's   #
// # own `class str` always was.                                            #
// #                                                                        #
// # Assignability is `assignable(&TypeNode, &TypeNode)`, which is          #
// # three-valued and abstains with `None` instead of guessing.             #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn is_clearly_incompatible(_arg_type: &str, _param_ann: &str) -> bool {
    panic!(
        "basilisk-checker: `is_clearly_incompatible` was DELETED because it decided \
         assignability by comparing two RENDERED TYPE SPELLINGS, splitting the \
         annotation on the `|` character to approximate unions. It panics because the \
         real implementation — `assignable(&TypeNode, &TypeNode)` on canonical types — \
         DOES NOT EXIST YET at this call site. Do not restore the name match and do \
         not return `false` in its place: `false` silences the rule while it still \
         reports as implemented."
    )
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

    let fields = positional_dataclass_fields(class_info);

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
    // Bail on parse errors — those are reported separately as BSK-0000.
    if crate::annotation::AnnotationResolver::for_module(module).is_none() {
        return;
    }
    let graph = basilisk_resolver::ClassGraph::new(&module.classes);

    // Build a map of (class definition site, method_name) → FunctionInfo.
    let method_map = super::shared::method_name_map(&module.functions);

    for call in &module.calls {
        // Only process constructor calls. Which class the callee names was
        // RESOLVED at the call site, so `Shorthand = Widget; Shorthand()`
        // checks `Widget`'s constructor and a callee whose name merely matches
        // a class checks nothing ([RESOLV-CANONICAL-BINDING]).
        let Some(class_info) = call.callee_class_site.and_then(|site| graph.at(site)) else {
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
        if !metaclass_passes_through(&graph, &method_map, class_info, module) {
            continue;
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
    let required_count = required_dataclass_field_count(class_info);

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
        check_dataclass_arg_types(class_info, call, source, path, diagnostics);
    }
}

/// Find the constructor method (`__new__` or `__init__`) to validate against.
///
/// Returns the `__new__` method if it exists and has a non-trivial signature
/// (not just `*args, **kwargs`), otherwise falls back to `__init__`.
fn find_constructor_method<'a>(
    class_info: &ClassInfo,
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&'a FunctionInfo>>,
) -> Option<&'a FunctionInfo> {
    // Keyed on the class's DEFINITION SITE, so a module that declares two
    // classes with the same name checks each call against its own class's
    // constructor instead of collapsing both into one entry.
    let site = class_info.name_span;

    // Try __new__ first
    if let Some(func) = method_map
        .get(&(site, "__new__"))
        .and_then(|new_methods| new_methods.first().copied())
    {
        return Some(func);
    }

    // Fall back to __init__
    method_map
        .get(&(site, "__init__"))
        .and_then(|init_methods| init_methods.first().copied())
}

/// Check calls to functional-form `NamedTuple` / `namedtuple` for argument count errors.
///
/// Validates:
/// - Too few positional args (below required field count, considering defaults)
/// - Too many positional args (above total field count)
fn check_namedtuple_calls(_module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
    // DELETED: this joined `CallSite::callee` to `NamedTupleDefInfo::lhs_name`
    // through a string map. Which factory result a call invokes is binding
    // identity, not the current spelling of either name.
    panic!(
        "basilisk-checker: `check_namedtuple_calls` was DELETED because it matched a \
         call to a NamedTuple definition by rendered name. Rebuild it on the resolved \
         value-definition site at the call offset; do not restore the map or return \
         without checking."
    )
    /*
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
    */
}
