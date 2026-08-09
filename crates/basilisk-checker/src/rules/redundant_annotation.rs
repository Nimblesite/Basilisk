//! Implements [BSK-0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! BSK-0050: Redundant type annotation warning.
//!
//! Emits a warning when a type annotation is redundant because the inferred type
//! exactly matches the declared type. This is Basilisk's headline differentiator
//! from other type checkers.
//!
//! ```python
//! x: int = 42        # BSK-0050 — annotation is redundant
//! y: str = "hello"   # BSK-0050 — annotation is redundant
//! z: float = 42      # NO warning — annotation adds information (widening)
//! ```

use crate::types::InferredType;
use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

/// The engine's type for the value at `span`, widened to annotation form —
/// `x: int = 5` reads as `int` against `int`, exactly what "the annotation
/// repeats what inference already knows" means ([TYPEINF-REDUNDANT]).
///
/// Only a value whose type is syntactically self-evident (a literal or a
/// display) can make an annotation REDUNDANT. A call's result type comes
/// from its callee, so annotating it adds information — and BSK-0003 demands
/// exactly that annotation, which BSK-0050 must never contradict.
fn oracle_widened(
    types: &super::shared::module_types::ModuleTypes<'_>,
    span: Option<basilisk_resolver::Span>,
) -> Option<InferredType> {
    use ruff_python_ast::Expr;
    let oracle = types.oracle()?;
    let span = span?;
    if !matches!(
        oracle.expr(span)?,
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
    ) {
        return None;
    }
    let ty = oracle.synth_span(span)?;
    crate::expr_type::is_fully_known(&ty).then(|| crate::expr_type::display_widened(&ty))
}

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0050",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0050",
};

/// Emits BSK-0050 for redundant type annotations.
// Implements [TYPEINF-REDUNDANT] — when the written annotation is identical to
// what inference would produce, the annotation is noise and BSK-0050 fires.
pub(crate) struct RedundantAnnotationWarning;

impl Rule for RedundantAnnotationWarning {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["redundancy", "style"],
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
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The declared type comes from the shared cascade
        // ([TYPEINF-ANNOTATION-RESOLUTION]): an annotation that is redundant
        // *through an alias* (`type Age = int` then `x: Age = 1`) is redundant
        // all the same, and a name we cannot resolve is gradual, never a guess.
        let Some(resolver) = types.annotations() else {
            return;
        };
        // Check module-level variables
        module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| {
                let annotation_text = extract_annotation(&module.source, var.name_span)?;

                // The value's type comes from the module's shared oracle.
                let inferred_type = oracle_widened(types, var.rhs_span)?;

                let declared_type = var
                    .annotation_span
                    .and_then(|span| resolver.resolve_span(span))
                    .or_else(|| resolver.resolve_text(annotation_text))?;

                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((var.name_span, var.name.clone(), annotation_text.to_owned()))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(
                    &name,
                    &annotation,
                    span,
                    &module.path,
                ));
            });

        // Check class attributes
        module
            .classes
            .iter()
            .filter(|class| !annotation_defines_field(class))
            .flat_map(|class| &class.attributes)
            .filter(|attr| attr.has_annotation && attr.has_value)
            .filter_map(|attr| {
                let annotation_text = extract_annotation(&module.source, attr.name_span)?;

                // The value's type comes from the module's shared oracle; a
                // class-body literal the oracle has no span for falls back to
                // the source-window inference until the resolver records
                // attribute value spans.
                let inferred_type = oracle_widened(types, attr.rhs_span)
                    .unwrap_or_else(|| infer_type_from_source(&module.source, attr.name_span));

                // Skip if inference still failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }

                let declared_type = attr
                    .annotation_span
                    .and_then(|span| resolver.resolve_span(span))
                    .or_else(|| resolver.resolve_text(annotation_text))?;

                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((
                        attr.name_span,
                        attr.name.clone(),
                        annotation_text.to_owned(),
                    ))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(
                    &name,
                    &annotation,
                    span,
                    &module.path,
                ));
            });
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
// ##########################################################################
// # DELETED BODY — `extract_annotation`. DO NOT RESTORE IT AND DO NOT      #
// # RETURN `None` IN ITS PLACE.                                            #
// #                                                                        #
// # It recovered the annotation by scanning ONE SOURCE LINE for two        #
// # punctuation marks:                                                     #
// #                                                                        #
// #   let colon_pos = line[name_offset..].find(": ")? + name_offset;       #
// #   let annotation_end = line[after_colon..].find('=')…                  #
// #                                                                        #
// # Neither is part of Python's grammar. The space after the colon is a    #
// # PEP 8 preference, so `n:int = 1` produced no annotation at all; an     #
// # `=` inside the annotation — `Literal["a=b"]`, an `Annotated` payload — #
// # truncated it; an annotation split across lines (a parenthesised union, #
// # a long `Callable`) was cut at the newline; and `: ` occurring earlier  #
// # on the line, as in a dict display or a lambda default, was taken for   #
// # the declaration's colon.                                               #
// #                                                                        #
// # `AnnAssign::annotation` is an `Expr` the parser already produced, with #
// # its own span. There is nothing to find.                                #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn extract_annotation(_source: &str, _name_span: basilisk_resolver::Span) -> Option<&str> {
    panic!(
        "basilisk-checker: `extract_annotation` was DELETED because it recovered a \
         declaration's annotation by searching ONE SOURCE LINE for the literal text \
         `\": \"` and cutting at the first `=`, so `n:int = 1` had no annotation and a \
         multi-line or `=`-containing one was truncated. It panics because the real \
         implementation — reading `StmtAnnAssign::annotation` from the AST — DOES NOT \
         EXIST YET. Do not restore the line scan and do not return `None` in its \
         place: `None` disables the rule while it still reports as implemented."
    )
}

/// Check if types match for BSK-0050 purposes (base scalar comparison).
fn types_match_for_w0050(inferred: &InferredType, declared: &InferredType) -> bool {
    use InferredType::{Bool, Bytes, Float, Int, LiteralString, None_, Str};

    // Only fire BSK-0050 for simple scalar types that are exactly equal. A
    // string literal infers as `LiteralString`, which an explicit `str`
    // annotation genuinely restates. Collection and other complex types are
    // never flagged: the annotation documents element/key/value/tuple shape the
    // literal alone does not make obvious, so it stays useful even when the
    // engine could infer it (see test_w0050_{list,dict,set,tuple}_literal_no_warning).
    matches!(
        (inferred, declared),
        (Int, Int)
            | (Str | LiteralString, Str)
            | (Float, Float)
            | (Bool, Bool)
            | (Bytes, Bytes)
            | (None_, None_)
    )
}

/// Infer a type from the assignment's source text when resolver inference fails.
///
/// Annotated class attributes carry `RhsKind::Other` (the resolver does not
/// classify the RHS of an `AnnAssign`), so BSK-0050 recovers the literal type from
/// the source line.  When the RHS is *not* a recognisable literal — a name
/// reference, call, or arbitrary expression — Basilisk genuinely cannot infer
/// its type, so we return `Unknown`.  Claiming the annotation is redundant in
/// that case would be a false positive (issue #83): the annotation supplies a
/// type the inference engine does not have.
// ##########################################################################
// # DELETED BODY — `infer_type_from_source`. DO NOT RESTORE IT AND DO NOT  #
// # RETURN `InferredType::Unknown` IN ITS PLACE.                           #
// #                                                                        #
// # Its name says what it did: it INFERRED A TYPE FROM SOURCE TEXT. It cut #
// # one line out of the file, split it at the first `=`, and classified    #
// # the remaining characters with Rust's own number parser and a handful   #
// # of quote tests:                                                        #
// #                                                                        #
// #   if value_text.parse::<i64>().is_ok()      { Int }                    #
// #   else if value_text.parse::<f64>().is_ok() { Float }                  #
// #   else if value_text.starts_with('"') && value_text.ends_with('"')     #
// #                                            { Str }                     #
// #   else if value_text == "True" || … == "False" { Bool }                #
// #   else if value_text.starts_with("b\"") …  { Bytes }                   #
// #                                                                        #
// # Rust's literal grammar is not Python's. `1_000` parses in both but     #
// # means the same only by luck; `0x1F` parses in NEITHER, so a hex        #
// # integer was `Unknown`; `1e3` was matched by `parse::<f64>` — correct   #
// # by accident — while `10j` (a complex literal) fell through. Python     #
// # spellings the tests never see: `r"x"`, `rb"x"`, `"""x"""`, implicit    #
// # adjacent-string concatenation, and any value wrapped in parentheses    #
// # across lines. And the split at the FIRST `=` mis-reads `x: int = y ==  #
// # z` and every augmented assignment.                                     #
// #                                                                        #
// # A literal's type is the kind of node the parser built for it:          #
// # `Expr::NumberLiteral`, `Expr::StringLiteral`, `Expr::BytesLiteral`,    #
// # `Expr::BooleanLiteral`, `Expr::NoneLiteral`.                           #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn infer_type_from_source(_source: &str, _name_span: basilisk_resolver::Span) -> InferredType {
    panic!(
        "basilisk-checker: `infer_type_from_source` was DELETED because it inferred a \
         value's TYPE by cutting one source line at the first `=` and classifying the \
         remaining characters with Rust's number parser and quote-prefix tests — a \
         second, wrong lexer for Python. It panics because the real implementation — \
         reading the literal `Expr` node the parser already built — DOES NOT EXIST \
         YET. Do not restore the character classification and do not return `Unknown` \
         in its place."
    )
}

/// Create diagnostic for redundant annotation warning
fn make_diagnostic_for_var(
    name: &str,
    annotation: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    warning_diagnostic_owned(
        CODE.clone(),
        format!(
            "Redundant type annotation: `{name}` is annotated `{annotation}` but the inferred type is identical"
        ),
        span,
        path,
        Some("Remove the redundant annotation to reduce noise".to_owned()),
        Some(
            "Basilisk warns about redundant annotations to encourage cleaner code".to_owned(),
        ),
    )
}

/// Whether annotated assignments in this class declare *fields* rather than
/// plain attributes (issue #110).
///
/// In a `@dataclass` (including `dataclass_transform` factories) the annotation
/// is what makes the assignment a field — removing it silently deletes the
/// field from the generated `__init__`. BSK-0050 must never call such an
/// annotation redundant.
fn annotation_defines_field(class: &basilisk_resolver::ClassInfo) -> bool {
    class.is_dataclass
}
