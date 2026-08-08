//! Permutation-test harness for [PERMTEST-PLAN]. See
//! docs/plans/CHECKER-PYTHON-PERMUTATION-PLAN.md#PERMTEST-PLAN
//!
//! Two oracles, no hand-authored expected-output files:
//!
//! * **Family A — invariant** ([PERMTEST-FAMILY-A]). Semantically identical
//!   Python spelled differently must produce an identical diagnostic multiset.
//!   The expectation is the canonical run itself, so nothing can be fitted.
//! * **Family B — directed** ([PERMTEST-FAMILY-B]). Python the typing spec
//!   declares ill-typed must produce at least one diagnostic; Python the spec
//!   declares well-typed must produce none. Stated as a relation, never as a
//!   literal diagnostic list, so it survives rule renaming and message edits.
//!
//! **Vocabulary constraint** ([PERMTEST-VOCABULARY]). Every source string
//! authored against this harness draws its identifiers from outside the 913
//! names the conformance suite defines, and prefers library symbols from
//! outside the 55 `typing`/`typing_extensions` symbols the suite imports.
//! Concepts with exactly one spelling (`TypeVar`, `Protocol`, `overload`, …)
//! are **quarantined, not exempt**: they appear only under an alias or an
//! alternate import form, never bare.
//!
//! The permutation variants are authored as explicit paired sources rather than
//! produced by rewriting text. A textual rewriter would be the same defect this
//! suite exists to find, one level up.

use super::common::run;
use basilisk_checker::Diagnostic;
use std::collections::BTreeMap;
use std::error::Error;

/// Diagnostics that report the *environment* rather than the code under test.
///
/// A test module is a single in-memory file with no project around it, so an
/// unresolved third-party import says nothing about the rule being exercised.
/// Everything else counts.
const ENVIRONMENTAL: &[&str] = &["imports_unresolved", "BSK-0152"];

fn is_environmental(diag: &Diagnostic) -> bool {
    ENVIRONMENTAL.contains(&diag.code.code)
}

/// The checker's verdict on `source`, with environmental noise removed.
pub fn analyse(source: &str) -> Result<Vec<Diagnostic>, Box<dyn Error>> {
    Ok(run(source)?
        .into_iter()
        .filter(|diag| !is_environmental(diag))
        .collect())
}

/// Diagnostic codes as a multiset — the spelling-independent shape of a run.
///
/// Spans and messages embed identifier text, so they differ under a lawful
/// alpha-rename; the set of *judgements* must not.
pub fn code_multiset(diags: &[Diagnostic]) -> BTreeMap<&str, usize> {
    diags.iter().fold(BTreeMap::new(), |mut acc, diag| {
        *acc.entry(diag.code.code).or_insert(0) += 1;
        acc
    })
}

fn render(diags: &[Diagnostic]) -> String {
    code_multiset(diags)
        .iter()
        .map(|(code, count)| format!("{code}×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A permutation class from [PERMTEST-FAMILIES], carried on every variant so a
/// failure names the property that broke.
#[derive(Clone, Copy, Debug)]
pub enum Class {
    /// A1–A5: whitespace, comments, line breaks, quote style, trailing commas.
    /// AST-equal modulo spans.
    Formatting,
    /// A6: `from typing import X as Y`. The language reference makes `as` bind
    /// the same object.
    ImportAlias,
    /// A7: `import typing` + `typing.X` vs `from typing import X`. Same symbol,
    /// different binding path.
    ImportForm,
    /// A8: consistent alpha-rename of classes, type variables, aliases, params.
    /// Same binding structure.
    Rename,
}

impl Class {
    const fn label(self) -> &'static str {
        match self {
            Self::Formatting => "A1-A5 formatting",
            Self::ImportAlias => "A6 import alias",
            Self::ImportForm => "A7 import form",
            Self::Rename => "A8 alpha-rename",
        }
    }
}

/// One semantics-preserving spelling of a canonical source.
pub struct Variant {
    pub class: Class,
    pub source: &'static str,
}

/// Declare an A6 import-alias variant.
#[must_use]
pub const fn aliased(source: &'static str) -> Variant {
    Variant {
        class: Class::ImportAlias,
        source,
    }
}

/// Declare an A7 import-form variant.
#[must_use]
pub const fn import_form(source: &'static str) -> Variant {
    Variant {
        class: Class::ImportForm,
        source,
    }
}

/// Declare an A8 alpha-rename variant.
#[must_use]
pub const fn renamed(source: &'static str) -> Variant {
    Variant {
        class: Class::Rename,
        source,
    }
}

/// Declare an A1–A5 formatting variant.
#[must_use]
pub const fn reformatted(source: &'static str) -> Variant {
    Variant {
        class: Class::Formatting,
        source,
    }
}

/// **Family A oracle.** Every variant must yield the canonical diagnostic
/// multiset.
///
/// The canonical run is the expectation; no literal is authored, so a rule that
/// changes its verdict when the source is respelled fails here and cannot be
/// made green by editing an expectation.
pub fn assert_invariant(
    case: &str,
    canonical: &str,
    variants: &[Variant],
) -> Result<(), Box<dyn Error>> {
    let expected = analyse(canonical)?;
    for variant in variants {
        let actual = analyse(variant.source)?;
        assert_eq!(
            code_multiset(&expected),
            code_multiset(&actual),
            "{case}: {} changed the verdict.\n  canonical: [{}]\n  permuted:  [{}]\n\
             A semantics-preserving permutation must not move a diagnostic. \
             See [PERMTEST-FAMILY-A].",
            variant.class.label(),
            render(&expected),
            render(&actual),
        );
    }
    Ok(())
}

/// **Family B oracle, positive.** Source the typing spec declares ill-typed must
/// draw at least one diagnostic.
///
/// Deliberately code-agnostic: which rule catches it is an implementation
/// detail, *that* it is caught is the spec obligation.
pub fn assert_rejected(case: &str, spec_reason: &str, source: &str) -> Result<(), Box<dyn Error>> {
    let diags = analyse(source)?;
    assert!(
        !diags.is_empty(),
        "{case}: no diagnostic. The typing spec makes this an error — {spec_reason}.\n\
         Source:\n{source}\nSee [PERMTEST-FAMILY-B]."
    );
    Ok(())
}

/// **Family B oracle, positive and specific.** As [`assert_rejected`], but the
/// diagnostic must come from a named rule. Use only where the spec obligation
/// maps to exactly one rule; prefer [`assert_rejected`] otherwise.
pub fn assert_rejected_by(
    case: &str,
    code: &str,
    spec_reason: &str,
    source: &str,
) -> Result<(), Box<dyn Error>> {
    let diags = analyse(source)?;
    assert!(
        diags.iter().any(|diag| diag.code.code == code),
        "{case}: expected `{code}`, got [{}]. Spec: {spec_reason}.\n\
         Source:\n{source}\nSee [PERMTEST-FAMILY-B].",
        render(&diags)
    );
    Ok(())
}

/// **Family B oracle, negative.** Source the typing spec declares well-typed
/// must draw no diagnostic at all — the false-positive side, which an
/// invariance oracle alone cannot see.
pub fn assert_accepted(case: &str, spec_reason: &str, source: &str) -> Result<(), Box<dyn Error>> {
    let diags = analyse(source)?;
    assert!(
        diags.is_empty(),
        "{case}: false positive [{}]. The typing spec makes this legal — {spec_reason}.\n\
         Source:\n{source}\nSee [PERMTEST-FAMILY-B].",
        render(&diags)
    );
    Ok(())
}

/// Both directions of one spec obligation, plus the permutations of each.
///
/// The common shape: an ill-typed program, its well-typed repair (B2), and the
/// respellings of both. A rule that fires on spelling fails the invariance leg;
/// a rule that does not analyse at all fails the directed leg.
pub struct SpecObligation<'a> {
    /// What the typing spec requires, in one clause. Appears in every failure.
    pub spec_reason: &'a str,
    /// Ill-typed under the spec.
    pub rejected: &'a str,
    /// Well-typed under the spec — usually `rejected` with the defect repaired.
    pub accepted: &'a str,
    /// Semantics-preserving respellings of `rejected`.
    pub rejected_variants: &'a [Variant],
    /// Semantics-preserving respellings of `accepted`.
    pub accepted_variants: &'a [Variant],
}

impl SpecObligation<'_> {
    /// Run every leg. Directed first: an invariance pass over a rule that never
    /// fires is vacuous, so the directed failure is the one worth reporting.
    pub fn assert(&self, case: &str) -> Result<(), Box<dyn Error>> {
        assert_rejected(case, self.spec_reason, self.rejected)?;
        assert_accepted(case, self.spec_reason, self.accepted)?;
        assert_invariant(case, self.rejected, self.rejected_variants)?;
        assert_invariant(case, self.accepted, self.accepted_variants)
    }
}
