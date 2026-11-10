// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   crate::subtyping (SubtypingContext::is_subtype / name_subtype)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! `assignment_compatibility`: Assignment type incompatibility (literal mismatches).
//! Owns structural `TypedDict` assignment for [TYPEINF-SUBTYPING-TYPEDDICT].
//!
//! Detects annotated module-level variables where the declared type and the
//! literal kind of the right-hand side are clearly incompatible, for example:
//!
//! ```python
//! count: int = "hello"   # str literal assigned to int annotation → E0014
//! label: str = 42        # int literal assigned to str annotation → E0014
//! flag:  bool = "yes"    # str literal assigned to bool annotation → E0014
//! ratio: float = "1.5"   # str literal assigned to float annotation → E0014
//! ```
//!
//! Every right-hand side — literal, call, constructor, method, variable — is
//! typed by the module's [`ModuleOracle`] ([NARROWPLAN-INTEGRATION] Step 1:
//! `BidirEngine::synth`, with `synth_call` resolving call returns, GitHub
//! #397/#378), collection displays are judged in the annotation's
//! expected-type context by engine check mode, and nominal verdicts route
//! through [`crate::subtyping::SubtypingContext`].

mod alias_match;
mod callable_check;
mod dataclass_check;
mod default_spec;
mod enum_expand;
#[expect(
    dead_code,
    reason = "AST scaffolding preserved for its rebuilt consumer ([ASTREBUILD-PHASE-RESOLVER]); the text-matched caller was deleted under [ASTREBUILD-LAW]"
)]
mod protocol_members;
mod sig_model;
mod sig_subtype;
mod skip_names;
mod tuple_check;
mod typeform_check;

use skip_names::SkipNames;

use crate::annotation::AnnotationResolver;
use crate::rules::shared::module_types::ModuleTypes;
use crate::rules::shared::oracle::ModuleOracle;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, Span, VariableInfo};
use ruff_python_ast::Expr;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

use dataclass_check::check_dataclass_attr_assignments;
use tuple_check::check_tuple_reassignments;

pub(crate) const CODE: ErrorCode = ErrorCode {
    code: "assignment_compatibility",
    docs_url: "https://www.basilisk-python.dev/errors/assignment_compatibility",
};

/// Emits `assignment_compatibility` for annotated module variables whose annotation and literal
/// RHS are obviously incompatible.
// Implements [TYPEINF-VARS-ANNOTATED] — the annotation is the declared type; the
// inferred RHS type must be assignable to it (e.g. `x: float = 42` is OK,
// `x: str = 42` is an error).
pub(crate) struct AssignmentTypeMismatch;

impl Rule for AssignmentTypeMismatch {
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
        let Some(resolver) = types.annotations() else {
            return;
        };
        let empty_params = ParamMaps::default();
        let skip = SkipNames::collect(module);
        let call_index = callable_check::build_index(module);
        let oracle = types.oracle();
        let nominal = types.nominal();
        check_vars(
            &module.module_vars,
            &module.source,
            &module.path,
            diagnostics,
            &empty_params,
            &skip,
            &module.functions,
            &call_index,
            resolver,
            oracle,
            nominal,
        );
        check_local_vars(
            module,
            diagnostics,
            &skip,
            &call_index,
            resolver,
            oracle,
            nominal,
        );
        check_tuple_reassignments(module, diagnostics);
        check_dataclass_attr_assignments(module, diagnostics);
        typeform_check::check_typeform_calls(module, resolver, diagnostics);
        default_spec::check_default_specializations(module, diagnostics);
    }
}

/// The engine's answer for the RHS expression, `Unknown` when the module did
/// not parse or the span names no expression — an unresolved right-hand side
/// never manufactures a diagnostic ([CHKARCH-CONFORMANCE-MODE]).
fn rhs_inferred(oracle: Option<&ModuleOracle<'_>>, var: &VariableInfo) -> InferredType {
    oracle
        .zip(var.rhs_span)
        .and_then(|(oracle, span)| oracle.synth_span(span))
        .unwrap_or(InferredType::Unknown)
}

/// Collection displays are checked in the annotation's expected-type context —
/// engine check mode carries the declared element types INWARD and judges each
/// element against them, the exact discipline `return`/`yield` positions use.
/// Bottom-up inference alone would type `{"k": x}` as
/// `dict[LiteralString, Unknown]` and reject it under dict invariance, so
/// `d: dict[str, str] = {"k": x}` would fire while the identical
/// `return {"k": x}` stays clean (GitHub #332). A genuine element mismatch
/// still falls through to the alias check, then to the diagnostic.
fn literal_collection_assignable(
    var: &VariableInfo,
    oracle: Option<&ModuleOracle<'_>>,
    inferred: &InferredType,
    declared: &InferredType,
    skip: &SkipNames,
) -> bool {
    let node = oracle.zip(var.rhs_span).and_then(|(o, span)| o.expr(span));
    let Some(display) = node else { return false };
    if !matches!(
        display,
        Expr::List(_) | Expr::Dict(_) | Expr::Set(_) | Expr::Tuple(_)
    ) {
        return false;
    }
    if oracle
        .zip(var.rhs_span)
        .and_then(|(o, span)| o.checks_span(span, declared))
        == Some(true)
    {
        return true;
    }
    let ctx = alias_match::AliasCtx {
        union: &skip.value_aliases,
        generic: &skip.generic_aliases,
    };
    alias_match::alias_assignable(inferred, declared, &ctx, 0)
}

/// `true` when the RHS is a surface the pre-engine rule already judged —
/// a literal, a display, an f-string, a lambda, or a name bound to an
/// annotated parameter. Every other surface (a call, an attribute, a name
/// with no annotation in scope) only became visible through the engine, and
/// the grounded-target abstention applies there so wider sight never turns
/// into a new false positive ([CHKARCH-CONFORMANCE-MODE]).
fn legacy_inference_surface(
    var: &VariableInfo,
    oracle: Option<&ModuleOracle<'_>>,
    params: &ParamMaps,
) -> bool {
    let node = oracle.zip(var.rhs_span).and_then(|(o, span)| o.expr(span));
    match node {
        Some(
            Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::FString(_)
            | Expr::List(_)
            | Expr::Dict(_)
            | Expr::Set(_)
            | Expr::Tuple(_)
            | Expr::Lambda(_),
        ) => true,
        Some(Expr::Name(name)) => params.annotations.contains_key(name.id.as_str()),
        _ => false,
    }
}

/// Is the declared type one this rule can pass judgment on? Structural
/// targets (`Protocol`, `TypedDict` — including inside unions/containers)
/// need member-level judgment a nominal comparison cannot give, and a nominal
/// leaf the module cannot ground (an unresolvable import, a `TypeVar` spelled
/// as a name) is a question, not an answer. Firing on either would be a false
/// positive on spec-valid code ([CHKARCH-CONFORMANCE-MODE]).
fn declared_target_judgeable(resolver: &AnnotationResolver<'_>, declared: &InferredType) -> bool {
    !resolver.is_structural_target(declared) && declared_target_grounded(resolver, declared)
}

// ##########################################################################
// # DELETED BODY — `declared_target_grounded`.                             #
// #                                                                         #
// # `InferredType::Named(name)` carries a RENDERING, and                    #
// # `AnnotationResolver::is_grounded_name(name)` reparsed that rendering   #
// # against the module's final namespace. The original occurrence's span,  #
// # scope, and binding were already gone. A groundedness verdict therefore #
// # changed after an unrelated rebinding or when the same class was reached #
// # through an alias.                                                       #
// ##########################################################################

/// DELETED — panics; nominal leaves must carry their resolved identity.
fn declared_target_grounded(_resolver: &AnnotationResolver<'_>, _declared: &InferredType) -> bool {
    panic!(
        "basilisk-checker: `declared_target_grounded` was DELETED because it \
         grounded `InferredType::Named` through a rendered name string. It panics \
         because the real implementation — checking the definition identity carried \
         by each nominal leaf — DOES NOT EXIST YET. Do not restore name reparsing and \
         do not choose a constant groundedness answer."
    )
}

// The nominal-subclass acceptance is the ONE shared judgment in
// `rules/shared/judge.rs` ([NARROWPLAN-INTEGRATION]: nominal verdicts route
// through `SubtypingContext`; one implementation, not two).
/// Annotation SPANS per annotated parameter of the enclosing function,
/// consumed by the structural callable-nominal rescue. Spans, not text —
/// the rescue resolves them to AST nodes ([ASTREBUILD-LAW]).
#[derive(Default)]
struct ParamMaps {
    annotations: std::collections::HashMap<String, Span>,
}

// ##########################################################################
// # DELETED BODY — `check_vars`. DO NOT RESTORE IT AND DO NOT RETURN EMPTY. #
// #                                                                         #
// # This was the assignment rule's verdict pipeline, and two of its gates  #
// # were explicitly spelling-dependent:                                   #
// #                                                                         #
// #   let annotation_text = extract_annotation(source, var.name_span)?;    #
// #   if annotation_text.starts_with('"') || ...                          #
// #   let name = annotation_text.trim().to_ascii_lowercase();              #
// #   alias_match::alias_value_assignable(..., name, ...)                  #
// #                                                                         #
// # `extract_annotation` scanned one source line for punctuation. The      #
// # lowercase alias key then made the annotation's SPELLING decide which   #
// # alias definition to expand. Quoting, case, formatting, qualification,  #
// # aliasing, and rebinding therefore changed the diagnostic before the    #
// # otherwise-resolved type relation ran. Its fallback also called         #
// # `resolve_text`, re-parsing a source rendering without its original      #
// # position or scope.                                                      #
// #                                                                         #
// # The lawful rebuild starts with each variable's annotation `Expr`, keeps #
// # resolved alias identity throughout, and renders text only after a       #
// # semantic mismatch is proven.                                           #
// ##########################################################################

/// DELETED — panics; see the banner above. The signature remains as the map
/// of callers that must be rebuilt around annotation AST nodes.
#[expect(
    clippy::too_many_arguments,
    reason = "the deleted verdict's callers still expose every context dependency needed by the AST rebuild"
)]
fn check_vars(
    _vars: &[VariableInfo],
    _source: &str,
    _path: &str,
    _diagnostics: &mut Vec<Diagnostic>,
    _params: &ParamMaps,
    _skip: &SkipNames,
    _functions: &[basilisk_resolver::FunctionInfo],
    _call_index: &callable_check::CallIndex,
    _resolver: &AnnotationResolver<'_>,
    _oracle: Option<&ModuleOracle<'_>>,
    _nominal: &super::shared::nominal::NominalHierarchy<'_>,
) {
    panic!(
        "basilisk-checker: `assignment_compatibility::check_vars` was DELETED because \
         its verdict read annotations from SOURCE LINES, skipped quoted types by their \
         first character, lowercased rendered annotations to join alias tables, and \
         reparsed text when span resolution failed. It panics because the real \
         implementation — retaining each annotation's original `Expr` and resolved \
         alias/type identity through the comparison — DOES NOT EXIST YET. Do not \
         restore the text pipeline and do not silently return no diagnostics."
    )
}

/// Attempt to validate a flagged assignment as structurally compatible
/// callable nominal: the RHS must be a name bound to a parameter whose
/// annotation NODE, compared against the declared annotation NODE, passes
/// the callable subtype check. Both annotations are judged as resolved AST
/// nodes, never as source text ([ASTREBUILD-LAW]).
fn callable_rescue(
    var: &VariableInfo,
    params: &ParamMaps,
    call_index: &callable_check::CallIndex,
    oracle: Option<&ModuleOracle<'_>>,
) -> bool {
    let Some(oracle) = oracle else {
        return false;
    };
    let Some(Expr::Name(rhs)) = var.rhs_span.and_then(|span| oracle.expr(span)) else {
        return false;
    };
    let Some(rhs_annotation) = params
        .annotations
        .get(rhs.id.as_str())
        .and_then(|span| oracle.expr(*span))
    else {
        return false;
    };
    let Some(declared) = var.annotation_span.and_then(|span| oracle.expr(span)) else {
        return false;
    };
    callable_check::assignment_compatible(declared, rhs_annotation, call_index)
}

/// Check local variables in function bodies for type mismatches.
///
/// The engine's scope overlay types parameter references
/// (`x: Literal[False] = a` where `a: Literal[0]` compares by value); the
/// raw annotation texts feed only the structural callable-nominal rescue.
fn check_local_vars(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
    skip: &SkipNames,
    call_index: &callable_check::CallIndex,
    resolver: &AnnotationResolver<'_>,
    oracle: Option<&ModuleOracle<'_>>,
    nominal: &super::shared::nominal::NominalHierarchy<'_>,
) {
    let source = &module.source;
    for func in &module.functions {
        let params = build_param_maps(&func.parameters);
        check_vars(
            &func.local_vars,
            source,
            &module.path,
            diagnostics,
            &params,
            skip,
            &module.functions,
            call_index,
            resolver,
            oracle,
            nominal,
        );
    }
}

/// Annotation span per annotated parameter, for the structural
/// callable-nominal rescue ([`callable_rescue`]).
fn build_param_maps(params: &[basilisk_resolver::ParameterInfo]) -> ParamMaps {
    let mut maps = ParamMaps::default();
    for param in params {
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let _ = maps.annotations.insert(param.name.clone(), ann_span);
    }
    maps
}

// ##########################################################################
// # DELETED BODIES — `nominal_name` / `nominal_key`.                       #
// #                                                                         #
// # These converted a nominal type back into a lowercase STRING, then      #
// # split that rendering at `[` to manufacture the lookup key used by      #
// # TypedDict skip tables. Case-folding merges distinct Python identifiers; #
// # bracket splitting is a second type parser; aliases and qualified names #
// # never share keys even when they resolve to one definition.              #
// #                                                                         #
// # TypedDict identity and schema membership must be keyed by definition   #
// # site, not by `InferredType::Named`'s display text.                      #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn nominal_name(_ty: &InferredType) -> Option<String> {
    panic!(
        "basilisk-checker: `nominal_name` was DELETED because it lowercased a \
         nominal type's RENDERING to create a semantic lookup key. It panics because \
         the real implementation — a resolved definition identity — DOES NOT EXIST \
         YET. Do not restore case-folding and do not return `None` in its place."
    )
}

/// DELETED — panics; see the banner above.
fn nominal_key(_ty: &InferredType) -> Option<String> {
    panic!(
        "basilisk-checker: `nominal_key` was DELETED because it extracted a type's \
         supposed identity by splitting its RENDERING at `[`. It panics because the \
         real implementation — resolved nominal and specialization structure — DOES \
         NOT EXIST YET. Do not restore the split and do not return `None`."
    )
}

/// `true` when a dict-literal assignment to a `TypedDict` annotation should
/// be skipped (field-level checking is E0093's job). The RHS is judged by its
/// AST node, never by sniffing source text.
fn typeddict_literal_skipped(
    _var: &VariableInfo,
    _oracle: Option<&ModuleOracle<'_>>,
    _declared_type: &InferredType,
    _skip: &SkipNames,
) -> bool {
    panic!(
        "basilisk-checker: `typeddict_literal_skipped` was DELETED because it \
         identified the declared TypedDict by a lowercased, bracket-stripped RENDERED \
         NAME and looked that string up in a name-keyed set. It panics because the real \
         implementation — matching the resolved TypedDict definition site — DOES NOT \
         EXIST YET. Do not restore the name set and do not choose a default skip verdict."
    )
}

/// `true` when an `extra_items=` `TypedDict` is assigned to a `dict[...]`
/// annotation — assignability depends on PEP 728 value types, which the
/// name-level comparison cannot evaluate.
fn extra_items_dict_skipped(
    _declared_type: &InferredType,
    _inferred_type: &InferredType,
    _skip: &SkipNames,
) -> bool {
    panic!(
        "basilisk-checker: `extra_items_dict_skipped` was DELETED because it \
         identified a PEP 728 TypedDict by a lowercased, bracket-stripped RENDERED NAME. \
         It panics because the real implementation — the inferred value's resolved \
         TypedDict definition and schema — DOES NOT EXIST YET. Do not restore the name \
         lookup and do not choose a default skip verdict."
    )
}

/// Create diagnostic for inference-based type mismatch.
fn make_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    inferred: &InferredType,
    declared: &InferredType,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Type mismatch: `{}` is annotated `{annotation}` ({}) but assigned {}",
            var.name, declared, inferred
        ),
        var.name_span,
        path,
        Some(format!(
            "Either change the annotation to match the value, or change the value to `{annotation}`"
        )),
        Some(
            "Basilisk requires the inferred type to be assignable to the declared type".to_owned(),
        ),
    )
}

// ##########################################################################
// # DELETED BODY — `extract_annotation`. DO NOT RESTORE IT.                #
// #                                                                         #
// # It found an annotation by scanning one SOURCE LINE for the literal     #
// # punctuation `": "`, then cut at the first `=`. A missing style-space, #
// # a multiline annotation, or `=` inside `Literal[...]` changed the type  #
// # the checker believed was declared. `StmtAnnAssign::annotation` and its #
// # span already exist in the parser AST.                                  #
// ##########################################################################

/// DELETED — panics; callers must retain the original annotation `Expr`.
pub(super) fn extract_annotation(_source: &str, _name_span: Span) -> Option<&str> {
    panic!(
        "basilisk-checker: `assignment_compatibility::extract_annotation` was \
         DELETED because it recovered a type annotation by scanning SOURCE TEXT for \
         punctuation on one line. It panics because the real implementation — the \
         original annotation `Expr` and span — DOES NOT EXIST YET at these callers. Do \
         not restore the line scan and do not return `None` in its place."
    )
}
