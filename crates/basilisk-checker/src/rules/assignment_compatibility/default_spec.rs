//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! PEP 696 default-specialization mismatch for bare generic class assignments.
//!
//! A bare reference to a generic class whose remaining free type parameters
//! all carry PEP 696 defaults is equivalent to the class specialized with
//! those defaults.  Assigning the bare class to a `type[C[Arg]]` annotation
//! is therefore an error when `Arg` differs from the parameter's default:
//!
//! ```python
//! class SubclassMe(Generic[T1, DefaultStrT]): ...
//! class Bar(SubclassMe[int, DefaultStrT]): ...
//!
//! x1: type[Bar[str]] = Bar  # OK  — DefaultStrT defaults to str
//! x2: type[Bar[int]] = Bar  # E   — bare Bar specializes to Bar[str]
//! ```
//!
//! Every verdict is computed on resolved AST nodes ([ASTREBUILD-LAW]): the
//! annotation is destructured structurally (its `type` base recognised by
//! what it RESOLVES to), and the requested argument is related to the
//! `TypeVar`'s lowered default through [`equivalent`]
//! ([RESOLV-CANONICAL-RELATION]).  A diagnostic is emitted only on a
//! definite `Some(false)`; unresolvable nodes abstain.

use std::collections::HashMap;

use basilisk_resolver::{
    BuiltinClass, ResolvedModule, Span, TypeNode, TypeVarCallInfo, VariableInfo,
};
use ruff_python_ast::Expr;
use ruff_text_size::Ranged as _;

use crate::diagnostic::Diagnostic;
use crate::rules::shared::{parse_module, ExprIndex};
use crate::span_util::slice_span;

/// Check module-level and function-local annotated variables for
/// `x: type[C[Args]] = C` assignments where a defaulted type parameter's
/// default conflicts with the requested specialization.
pub(super) fn check_default_specializations(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parsed) = parse_module(module) else {
        return;
    };
    let index = ExprIndex::build(&parsed.ast);
    let defaults = typevar_defaults(module, &index);
    if defaults.is_empty() {
        return;
    }

    let vars = module.module_vars.iter().chain(
        module
            .functions
            .iter()
            .flat_map(|func| func.local_vars.iter()),
    );
    for var in vars {
        check_var(var, module, &index, &defaults, diagnostics);
    }
}

/// DELETED — panics; see the banner below.
fn typevar_defaults<'m>(
    _module: &'m ResolvedModule,
    _index: &ExprIndex<'_>,
) -> HashMap<&'m str, (TypeNode, &'m str)> {
    panic!(
        "basilisk-checker: `default_spec::typevar_defaults` was DELETED because it keyed \
         each PEP 696 default by the `TypeVar`'s BOUND NAME, so the lookup that decides \
         this rule joined a class's parameter to a `TypeVar` by spelling. It panics \
         because the real implementation — keying on `TypeVarCallInfo::span`, the call \
         expression's own identity — DOES NOT EXIST YET. Do not restore the name key and \
         do not return an empty map in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `typevar_defaults`. DO NOT RESTORE IT AND DO NOT RETURN  #
// # AN EMPTY MAP.                                                           #
// #                                                                         #
// #   Some((tv.name.as_str(), (node, display)))                             #
// #   ... later: defaults.get(param_name.as_str())                          #
// #                                                                         #
// # A `TypeVar` IS NOT ITS NAME. `TypeVarCallInfo::span` is the range of     #
// # the `TypeVar(...)` call expression, and that is the identity of the type #
// # variable — it is what an assignment binds and what every alias of that   #
// # binding leads back to. Keying on `TypeVarCallInfo::name` instead means:  #
// #                                                                         #
// #   * two `TypeVar` calls in one module bound to different names but       #
// #     written `TypeVar("T", default=int)` and `TypeVar("T", default=str)`  #
// #     collapse onto one entry, and whichever the iteration reached last    #
// #     decides the diagnostic;                                              #
// #   * `T = TypeVar("T", default=str); Alias = T` — a class parameterised   #
// #     on `Alias` finds no default and the rule silently stops checking.    #
// #                                                                         #
// # The name in `TypeVar("T")`'s first argument is a RUNTIME LABEL. It is    #
// # not required to match the variable it is bound to and nothing about the  #
// # type system reads it.                                                    #
// #                                                                         #
// # The rebuild keys this map on `TypeVarCallInfo::span` and requires        #
// # `free_type_params` — already deleted, directly above the `check_var`     #
// # loop — to yield parameter IDENTITIES rather than rendered names, so the  #
// # two sides have something lawful to join on. `check_var` is kept as the   #
// # map of what reads this.                                                  #
// ##########################################################################

/// The lowered `default=` argument of a recorded `TypeVar(...)` call, with the
/// source text of the expression it came from.
///
/// The expression is found on the call NODE and lowered through the module's
/// bindings — never read back from source text ([ASTREBUILD-LAW]). The
/// returned `&str` is that expression's own span, for message rendering.
#[expect(
    dead_code,
    reason = "the name-keyed PEP 696 verdict was deleted; this AST lowering is retained for the identity-based rebuild"
)]
fn default_node<'m>(
    tv: &TypeVarCallInfo,
    index: &ExprIndex<'_>,
    module: &'m ResolvedModule,
) -> Option<(TypeNode, &'m str)> {
    let Expr::Call(call) = index.expr(tv.span)? else {
        return None;
    };
    // A keyword argument's name is fixed syntax at the call site: it cannot be
    // imported, aliased, or rebound, so reading it is not a spelling test on a
    // type ([ASTREBUILD-LAW]).
    let default = call
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|arg| arg.as_str() == "default"))?;
    let display = slice_span(&module.source, Span::from(default.value.range()))?;
    Some((TypeNode::lower(&module.bindings, &default.value), display))
}

// ##########################################################################
// # DELETED BODY — `check_var`. DO NOT RESTORE IT AND DO NOT RETURN EARLY. #
// #                                                                         #
// # The class join itself had been repaired to compare definition sites,   #
// # but the decisive TypeVar/default join remained:                        #
// #                                                                         #
// #   let free_params = free_type_params(class_info, module);              #
// #   defaults.get(param_name.as_str())                                    #
// #                                                                         #
// # Both sides are RENDERED NAMES. `free_type_params` produced spellings   #
// # harvested from base subscripts and `typevar_defaults` keyed defaults   #
// # by `TypeVarCallInfo::name`. An alias of a TypeVar therefore lost its    #
// # default, while distinct TypeVar objects carrying the same runtime label #
// # collided. The later `equivalent(TypeNode, TypeNode)` comparison cannot #
// # repair a wrong spelling-based join.                                    #
// #                                                                         #
// # The lawful implementation joins both maps on `TypeVarCallInfo::span`,  #
// # reached by resolving each original base-argument `Expr`.               #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn check_var(
    _var: &VariableInfo,
    _module: &ResolvedModule,
    _index: &ExprIndex<'_>,
    _defaults: &HashMap<&str, (TypeNode, &str)>,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `default_spec::check_var` was DELETED because its \
         PEP 696 verdict joined a class parameter to a TypeVar default through two \
         RENDERED NAME strings. It panics because the real implementation — resolving \
         each parameter expression to `TypeVarCallInfo::span` and joining defaults on \
         that identity — DOES NOT EXIST YET. Do not restore the string-keyed maps and \
         do not return without checking in its place."
    )
}

/// Destructure a `type[C[args…]]` annotation NODE: the outer base must
/// denote the builtin `type` — recognised by LOWERING it through the
/// module's bindings, so `typing.Type`, an aliased import, or any other
/// spelling behaves identically ([ASTREBUILD-LAW]) — the inner base must be
/// a plain name, and the returned args are the inner subscript's elements.
#[expect(
    dead_code,
    reason = "the name-keyed PEP 696 verdict was deleted; this AST destructuring is retained for the identity-based rebuild"
)]
fn type_of_subscript<'e>(
    annotation: &'e Expr,
    module: &ResolvedModule,
) -> Option<(&'e Expr, Vec<&'e Expr>)> {
    let Expr::Subscript(outer) = annotation else {
        return None;
    };
    if TypeNode::lower(&module.bindings, &outer.value) != TypeNode::Builtin(BuiltinClass::Type) {
        return None;
    }
    let Expr::Subscript(inner) = outer.slice.as_ref() else {
        return None;
    };
    // The inner base must be a plain name; the NODE is returned so the
    // caller can resolve it through the binding table rather than compare
    // its spelling.
    let class_ref = inner.value.as_ref();
    if !matches!(class_ref, Expr::Name(_)) {
        return None;
    }
    let args = match inner.slice.as_ref() {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    };
    Some((class_ref, args))
}

// ##########################################################################
// # DELETED BODY — `free_type_params`. DO NOT RESTORE IT.                  #
// #                                                                         #
// #   let typevar_names = collect_name_set(&module.typevar_calls);         #
// #   base.type_arg_names.iter().filter(|n| typevar_names.contains(n))     #
// #                                                                         #
// # A class's type parameters were identified by matching the RENDERED     #
// # names in its base subscripts against the set of names `TypeVar`s were  #
// # declared with. The result is a `Vec<String>` that `typevar_defaults`   #
// # is then keyed by, so PEP 696 default checking is a spelling join end   #
// # to end:                                                                 #
// #                                                                         #
// #   * `Param = T; class C(Base[Param])` records `"Param"`, which matches #
// #     no `TypeVar` entry, and every default check on `C` vanishes;       #
// #   * a class attribute or import merely SPELLED like a `TypeVar` is     #
// #     counted as a type parameter;                                        #
// #   * two `TypeVar`s spelled alike in one module collapse to one entry.  #
// #                                                                         #
// # The lawful replacement resolves each base subscript ARGUMENT through   #
// # the binding table and keeps `TypeVarCallInfo::span` — the construction #
// # itself — as the parameter's identity, which is what                    #
// # `BindingTable::local_value_binding` reaches through any alias chain.   #
// ##########################################################################

/// DELETED — panics; see the banner above.
#[expect(
    dead_code,
    reason = "the name-keyed PEP 696 caller was deleted; this panic shell remains as the rebuild boundary"
)]
fn free_type_params(
    _class_info: &basilisk_resolver::ClassInfo,
    _module: &ResolvedModule,
) -> Vec<String> {
    panic!(
        "basilisk-checker: `free_type_params` was DELETED because it identified a \
         class's type parameters by matching RENDERED names from its base subscripts \
         against the names `TypeVar`s were declared with. It panics because the real \
         implementation — base subscript arguments resolved to `TypeVarCallInfo::span` \
         through the binding table — DOES NOT EXIST YET. Do not restore the name set and \
         do not return an empty vector in its place."
    )
}
