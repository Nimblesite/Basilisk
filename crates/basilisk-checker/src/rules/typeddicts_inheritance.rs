//! Implements [`typeddicts_inheritance`] from [CHKARCH-DIAG-OWNERSHIP] and
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! `typeddicts_inheritance`: Invalid `TypedDict` inheritance.
//!
//! PEP 589 and the typing spec place constraints on `TypedDict` inheritance:
//!
//! 1. A `TypedDict` cannot inherit from both a `TypedDict` and a non-TypedDict
//!    base class (except `Generic`).
//!
//! 2. A `TypedDict` subclass cannot change the type of a field declared in a
//!    parent `TypedDict` class. PEP 705 refines this for the `ReadOnly`,
//!    `Required`, and `NotRequired` qualifiers:
//!    - A writable (non-`ReadOnly`) item may not be redeclared `ReadOnly`.
//!    - A required item may not be redeclared as not-required.
//!    - A writable item's value type is invariant; a `ReadOnly` item's value
//!      type may be narrowed to a subtype.
//!
//! 3. Multiple `TypedDict` inheritance is not allowed when two bases declare
//!    the same field with conflicting types or qualifiers.
//!
//! # What this rule ACTUALLY checks
//!
//! Only constraint 1. `Rule::check` calls `check_mixed_bases` and nothing
//! else — constraints 2 and 3 are described above and NOT implemented. A
//! module that redeclares a parent field with an incompatible type, or that
//! inherits two `TypedDict`s declaring the same field with conflicting types,
//! passes this rule in silence.
//!
//! The list above is the PEP's obligation, not a claim about coverage, and it
//! is kept because it is the specification of what still has to be built. Do
//! not read a green result from this rule as any evidence about 2 or 3.

use basilisk_resolver::{ClassGraph, ClassInfo, ResolvedBase, ResolvedModule, Span, TypingForm};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_inheritance",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_inheritance",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        None,
        Some("PEP 589: TypedDict subclassing has strict field-compatibility requirements"),
    )
}

/// Whether a base that resolves to a specification form may accompany
/// `TypedDict` bases.
///
/// PEP 589 permits exactly the implicit top type and `Generic`; `TypedDict`
/// itself is the declaration, not a mixed-in base. Every other form is a real
/// class and mixing it in is the error this rule reports.
///
/// This is a comparison between RESOLVED forms, not spellings: `object`
/// reached under any name is the same top type, and a module that defines its
/// own `class object` does not get the exemption — that base resolves to a
/// local class, never to [`TypingForm::ObjectClass`].
fn form_may_accompany_typeddict(form: TypingForm) -> bool {
    matches!(
        form,
        TypingForm::ObjectClass | TypingForm::Generic | TypingForm::TypedDict
    )
}

/// Report every base of `cls` that is a class but not a `TypedDict`.
///
/// REBUILT on resolved base identity. The deleted version exempted the top
/// type with `EXEMPT: &["object"]` and asked a name-keyed map whether each
/// base was a `TypedDict`; both answers moved with the spelling of the base.
/// Implements [RESOLV-CANONICAL-BINDING].
fn check_mixed_bases(
    graph: &ClassGraph<'_>,
    cls: &ClassInfo,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for base in &cls.resolved_bases {
        let offending = match base.resolved {
            // A class this module defines: an error unless it is itself a
            // TypedDict, transitively.
            ResolvedBase::LocalClass(site) => graph
                .at(site)
                .is_some_and(|base| !graph.is_typed_dict(base)),
            ResolvedBase::Form(form) => !form_may_accompany_typeddict(form),
            // A base this module cannot see is not evidence of anything.
            ResolvedBase::Unknown => false,
        };
        if !offending {
            continue;
        }
        // Message text only — the verdict above came from the resolved base.
        let written = base.span.slice_source(source).unwrap_or("<base>");
        diagnostics.push(make_diagnostic(
            format!(
                "`{}` is a TypedDict and cannot also inherit from `{written}`, \
                 which is not a TypedDict",
                cls.name
            ),
            base.span,
            path,
        ));
    }
}

/// Emits `typeddicts_inheritance` for invalid `TypedDict` inheritance.
pub(crate) struct InvalidTypedDictInheritance;

impl Rule for InvalidTypedDictInheritance {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let graph = ClassGraph::new(&module.classes);

        for cls in graph.typed_dicts() {
            check_mixed_bases(&graph, cls, &module.source, &module.path, diagnostics);
        }
    }
}
