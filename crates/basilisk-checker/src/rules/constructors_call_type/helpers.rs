//! Implements [`constructors_call_type`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper types and functions for `constructors_call_type`.
//!
//! Contains constructor signature resolution, argument type checking,
//! and shared AST utilities used by the main rule.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{assignable, BindingTable, Span, TypeNode};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::ExprIndex;
use crate::span_util::{node_message_text, slice_span};

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "constructors_call_type",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_call_type",
};

// ---------------------------------------------------------------------------
// TypeVar bound map
// ---------------------------------------------------------------------------

// ##########################################################################
// # DELETED BODY — `build_typevar_bound_map`. DO NOT RESTORE IT.           #
// #                                                                         #
// #   let var_name = expr_simple_name(target)?;                            #
// #   if !typevar_names.contains(&var_name) { continue }                   #
// #   map.insert(var_name, expr_simple_name(&kw.value)?)                   #
// #                                                                         #
// # Both halves of every entry are RENDERED NAMES: the `TypeVar` is        #
// # identified by the word its assignment target is spelled with, and its  #
// # bound by the word the `bound=` expression is spelled with. The map is  #
// # then consumed by name, so:                                             #
// #                                                                         #
// #   * `Marker = Cairn; TypeVar("T", bound=Marker)` recorded `"Marker"`,  #
// #     which matches no class and silently drops every judgment;          #
// #   * `bound=mod.Cairn` recorded nothing at all;                         #
// #   * an unrelated local class merely spelled like the bound matched.    #
// #                                                                         #
// # The lawful replacement keys on `TypeVarCallInfo::span` (the `TypeVar`  #
// # construction itself, which `local_value_binding` reaches through any   #
// # number of aliases) and stores the bound's DEFINITION SITE from         #
// # `local_class_definition`.                                              #
// #                                                                         #
// # Pinned by: tests/constructor_identity_tests.rs                          #
// ##########################################################################

/// DELETED — panics; see the banner above.
pub(super) fn build_typevar_bound_map<'src>(
    _stmts: &'src [Stmt],
    _typevar_names: &[&str],
) -> HashMap<&'src str, &'src str> {
    panic!(
        "basilisk-checker: `build_typevar_bound_map` was DELETED because it identified \
         both a `TypeVar` and its `bound=` class by the WORDS they were spelled with, \
         and its consumers joined on those words. It panics because the real \
         implementation — keyed on `TypeVarCallInfo::span` with the bound resolved to a \
         definition site — DOES NOT EXIST YET. Do not restore the name lookup and do not \
         return an empty map in its place."
    )
}

// ---------------------------------------------------------------------------
// Constructor signature resolution
// ---------------------------------------------------------------------------

/// The resolved arity contract for a class constructor.
#[derive(Debug)]
pub(super) enum ConstructorSig {
    /// No non-self arguments (e.g. bare `object.__init__`).
    NoArgs,
    /// The constructor requires `min..=max` non-self arguments.
    Required { min: usize, max: usize },
    /// Cannot determine (varargs / kwargs present) — anything is OK.
    Unknown,
}

/// Resolve the constructor argument signature for a class by inspecting its
/// metaclass `__call__`, `__new__`, or `__init__` (in that priority order).
#[expect(
    dead_code,
    reason = "orphaned by the deleted name-keyed `check_type_call`; retained for the identity-based rebuild"
)]
pub(super) fn resolve_constructor_sig(
    graph: &basilisk_resolver::ClassGraph<'_>,
    class_info: &basilisk_resolver::ClassInfo,
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&basilisk_resolver::FunctionInfo>>,
) -> ConstructorSig {
    // 1. Check metaclass __call__ first.
    if let Some(meta_sig) = check_metaclass_call(graph, class_info, method_map) {
        return meta_sig;
    }

    // 2/3/4. `__new__`, then `__init__`, on the class itself and then up its
    // resolved base chain.
    //
    // REBUILT in one respect: the walk used to be over `class_bases`, which
    // took each base's head by splitting SOURCE TEXT at `[`, and it needed a
    // `base_name == "object"` skip to stop the top type looking like a local
    // class. `ClassGraph::ancestors` yields definition-site-keyed local
    // classes, so `object` — not a class this module defines — simply is not
    // among them and no spelling comparison is needed.
    //
    // NOT AN MRO, despite what this comment used to call it. `ancestors` is a
    // cycle-safe depth-first REACHABILITY order; Python resolves methods by C3
    // linearisation. The two disagree under multiple inheritance, so `the
    // first ancestor declaring __new__` can be a different definition from the
    // one Python would call, and the arity reported below is then the wrong
    // signature's. Single inheritance — where the orders coincide — is the
    // only shape this is right for by construction.
    //
    // The fall-through is also unsound in the other direction: reaching the
    // end of the walk yields `ConstructorSig::NoArgs`, which asserts that NO
    // constructor exists anywhere. That holds only if the ancestry was walked
    // completely. `ClassGraph::ancestry` reports whether it was
    // (`Ancestry::complete`); this walk uses `ancestors`, which discards that
    // flag, so a class whose base is imported from a module the checker never
    // read is treated as having no `__init__` and every argument passed to it
    // is reported as excess.
    for ancestor in graph.ancestors(class_info) {
        if let Some(new_sig) = method_map.get(&(ancestor.name_span, "__new__")) {
            return sig_from_funcs(new_sig);
        }
        if let Some(init_sig) = method_map.get(&(ancestor.name_span, "__init__")) {
            return sig_from_funcs(init_sig);
        }
    }

    // Default: object() — no args.
    ConstructorSig::NoArgs
}

/// The constructor signature imposed by the class's metaclass `__call__`, if
/// this module defines that metaclass and it declares one.
///
/// REBUILT on resolved identity. The deleted version read:
///
/// ```ignore
/// let meta_name = class_info.metaclass_name.as_deref()?;
/// class_map.get(meta_name);
/// method_map.get(&(meta_name, "__call__"))
/// ```
///
/// `metaclass_name` is the RENDERED text of a `metaclass=` value, filled only
/// when that value is a bare word. The metaclass, and the owner of its
/// `__call__`, were both identified by that string, so `metaclass=mod.Meta`
/// found nothing, an aliased metaclass found nothing, and a local class merely
/// sharing the rendered name was used instead — and the constructor signature
/// is what every argument check is measured against.
///
/// `ClassInfo::metaclass_site` is the same expression resolved through the
/// binding table, and the method index is keyed on definition site, so both
/// halves now name the class rather than describe it.
fn check_metaclass_call(
    graph: &basilisk_resolver::ClassGraph<'_>,
    class_info: &basilisk_resolver::ClassInfo,
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&basilisk_resolver::FunctionInfo>>,
) -> Option<ConstructorSig> {
    let meta_site = class_info.metaclass_site?;
    // The metaclass must be a class this module defines for its body to be
    // visible here; anything else abstains ([CHKARCH-CONFORMANCE-MODE]).
    let meta = graph.at(meta_site)?;
    // `__call__` may be inherited by the metaclass itself.
    graph
        .ancestors(meta)
        .into_iter()
        .find_map(|ancestor| method_map.get(&(ancestor.name_span, "__call__")))
        .map(|funcs| sig_from_funcs(funcs))
}

/// Derive a `ConstructorSig` from one or more `FunctionInfo` entries.
pub(super) fn sig_from_funcs(funcs: &[&basilisk_resolver::FunctionInfo]) -> ConstructorSig {
    if let Some(func) = funcs.first() {
        // If it has *args or **kwargs, we can't know the exact arity.
        if func.vararg.is_some() || func.kwarg.is_some() {
            return ConstructorSig::Unknown;
        }
        // Skip the first parameter (self / cls).
        let params: Vec<&basilisk_resolver::ParameterInfo> =
            func.parameters.iter().skip(1).collect();
        let min = params.iter().filter(|p| !p.has_default).count();
        let max = params.len();
        return ConstructorSig::Required { min, max };
    }
    ConstructorSig::NoArgs
}

// ##########################################################################
// # `class_bases` IS GONE. DO NOT RECREATE IT.                            #
// #                                                                       #
// # It took each base's head by splitting its SOURCE TEXT at `[`, so base #
// # identity moved with formatting and aliasing, and it needed a          #
// # `base_name == "object"` companion check to stop the top type looking  #
// # like a local class. The replacement is `ClassGraph::ancestors`, which #
// # yields the classes themselves, keyed on definition site.              #
// #                                                                       #
// # The same defect existed in `constructors_call_init::all_base_names`.  #
// ##########################################################################

// ---------------------------------------------------------------------------
// Keyword and positional argument type checking
// ---------------------------------------------------------------------------

/// Check that keyword arguments match the expected parameter types.
///
/// The literal argument is typed with [`TypeNode::of_literal_expr`] and
/// related to the parameter's annotation lowered through the module's binding
/// table ([ASTREBUILD-LAW]); a diagnostic is emitted only on a proven
/// `Some(false)`. Annotation source text appears in the MESSAGE only.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
pub(super) fn check_kwarg_types(
    call: &ast::ExprCall,
    class_site: basilisk_resolver::Span,
    class_name: &str,
    kw_names: &[&str],
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    bindings: &BindingTable,
    index: &ExprIndex<'_>,
    source: &str,
    path: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the constructor function (prefer __new__ then __init__).
    let func = find_constructor_func(class_site, method_map);
    let Some(func) = func else { return };

    for kw in &call.arguments.keywords {
        let Some(kw_name) = kw.arg.as_deref() else {
            continue;
        };
        // Find the matching parameter.
        let Some(param) = func.parameters.iter().skip(1).find(|p| p.name == kw_name) else {
            continue;
        };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        // Annotations the index cannot map to a node abstain
        // ([ASTREBUILD-PHASE-RESOLVER]).
        let Some(ann_expr) = index.expr(ann_span) else {
            continue;
        };
        let target = TypeNode::lower(bindings, ann_expr);
        if assignable(&TypeNode::of_literal_expr(&kw.value), &target) == Some(false) {
            let expected_type = slice_span(source, ann_span)
                .unwrap_or("<annotation>")
                .trim();
            let arg_text = node_message_text(source, &kw.value);
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Keyword argument `{kw_name}={arg_text}` is incompatible with \
                     parameter `{kw_name}: {expected_type}` of `{class_name}` constructor"
                ),
                span,
                path,
                Some(format!(
                    "Pass a `{expected_type}` value for keyword argument `{kw_name}`"
                )),
                None,
            ));
        }
    }
    // Suppress unused warning.
    let _ = kw_names;
}

/// Check positional arguments against the constructor parameter types.
///
/// Same relation as [`check_kwarg_types`]: literal arguments against lowered
/// annotations, emitting only on a proven `Some(false)`.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
pub(super) fn check_positional_arg_types(
    call: &ast::ExprCall,
    class_site: basilisk_resolver::Span,
    class_name: &str,
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    bindings: &BindingTable,
    index: &ExprIndex<'_>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let func = find_constructor_func(class_site, method_map);
    let Some(func) = func else { return };

    // Skip self/cls param.
    let params: Vec<&basilisk_resolver::ParameterInfo> = func.parameters.iter().skip(1).collect();

    for (idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(param) = params.get(idx) else { break };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        // Annotations the index cannot map to a node abstain
        // ([ASTREBUILD-PHASE-RESOLVER]).
        let Some(ann_expr) = index.expr(ann_span) else {
            continue;
        };
        let target = TypeNode::lower(bindings, ann_expr);
        if assignable(&TypeNode::of_literal_expr(arg_expr), &target) == Some(false) {
            let expected_type = slice_span(source, ann_span)
                .unwrap_or("<annotation>")
                .trim();
            let arg_text = node_message_text(source, arg_expr);
            let arg_span = Span::from(arg_expr.range());
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument {n} (`{arg_text}`) is incompatible with parameter `{pname}` of \
                     `{class_name}` constructor, which expects `{expected_type}`",
                    n = idx + 1,
                    pname = param.name,
                ),
                arg_span,
                path,
                Some(format!(
                    "Pass a `{expected_type}` value as argument {n}",
                    n = idx + 1
                )),
                None,
            ));
        }
    }
}

/// Find the primary constructor function for a class.
pub(super) fn find_constructor_func<'a>(
    class_site: basilisk_resolver::Span,
    method_map: &'a HashMap<
        (basilisk_resolver::Span, &str),
        Vec<&'a basilisk_resolver::FunctionInfo>,
    >,
) -> Option<&'a basilisk_resolver::FunctionInfo> {
    for method in &["__new__", "__init__"] {
        if let Some(func) = method_map
            .get(&(class_site, method))
            .and_then(|funcs| funcs.first())
        {
            return Some(func);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared AST helpers
// ---------------------------------------------------------------------------

/// If `expr` is a simple `Name` node, return its identifier string.
pub(super) fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}
