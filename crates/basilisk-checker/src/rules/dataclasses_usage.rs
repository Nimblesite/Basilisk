//! Implements [`dataclasses_usage`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `dataclasses_usage`: Type mismatch between a dataclass `field(default_factory=…)` and
//! the field's declared type annotation.
//!
//! When a dataclass field uses `field(default_factory=T)` where `T` is a known
//! callable that constructs instances of a simple built-in type, but the field's
//! annotation declares a different incompatible built-in type, Basilisk reports
//! an error.
//!
//! ```python
//! from dataclasses import dataclass, field
//!
//! @dataclass
//! class DC:
//!     a: int = field(default_factory=str)  # E: str() → str, not int
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_usage",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_usage",
};

/// Emits `dataclasses_usage` for dataclass fields whose `default_factory` type is
/// incompatible with the declared field annotation.
pub(crate) struct DataclassFieldDefaultFactoryMismatch;

impl Rule for DataclassFieldDefaultFactoryMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        for cls in &module.classes {
            if !cls.is_dataclass {
                continue;
            }
            for attr in &cls.attributes {
                if !attr.has_value {
                    continue;
                }
                let Some(ann_span) = attr.annotation_span else {
                    continue;
                };
                let Some(rhs_span) = attr.rhs_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(source, ann_span) else {
                    continue;
                };
                let Some(rhs_text) = slice_span(source, rhs_span) else {
                    continue;
                };

                let Some(factory_type) = extract_default_factory_type(rhs_text) else {
                    continue;
                };

                if is_factory_incompatible_with_annotation(factory_type, ann_text.trim()) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Field `{}` is annotated as `{}` but \
                             `default_factory={factory_type}` produces `{factory_type}` values",
                            attr.name,
                            ann_text.trim(),
                        ),
                        attr.name_span,
                        path,
                        Some(format!(
                            "Change the annotation to `{factory_type}` or use a compatible \
                             `default_factory`"
                        )),
                        Some(
                            "PEP 557: `default_factory` must be a zero-argument callable \
                             compatible with the field's declared type"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}

/// Extract the type name from a `dataclasses.field(default_factory=TypeName)`
/// initializer.
///
/// `field` requires an import from `dataclasses`, so recognising the call by
/// its source spelling is not import resolution.
//
// ##########################################################################
// # DELETED BODY. THE PREVIOUS DELETION LEFT `None` HERE, WHICH IS ITSELF  #
// # A PROTOCOL VIOLATION — CLAUDE.md: a deleted body "MUST be replaced     #
// # with a loud `panic!` and nothing else — never a default, `None`,       #
// # `false`, or empty result". `None` made a rule that cannot analyse      #
// # anything look like a rule that found nothing to report. Corrected to a #
// # panic here.                                                            #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn extract_default_factory_type(_rhs_text: &str) -> Option<&str> {
    panic!(
        "basilisk-checker: `extract_default_factory_type` was DELETED because it \
         recognised `dataclasses.field(default_factory=...)` from its SOURCE SPELLING. \
         It panics because the real implementation — resolving the callee and the \
         keyword's value through the binding table — DOES NOT EXIST YET. It previously \
         returned `None`, which is not a deletion: it silently reported 'no factory \
         found' for every dataclass in every file."
    )
}

// ##########################################################################
// # DELETED BODY — `is_factory_incompatible_with_annotation`. DO NOT       #
// # RESTORE IT AND DO NOT RETURN `false` IN ITS PLACE.                     #
// #                                                                        #
// # This is `name_subtype` — the construct the spelling guard forbids by   #
// # name — wearing a dataclasses hat. It settled TYPE COMPATIBILITY        #
// # between two RENDERED SPELLINGS:                                        #
// #                                                                        #
// #   let primary = field_ann.split('|').next()…      // union by chars    #
// #   let known = ["str", "int", "float", "bool", "bytes"];                #
// #   match factory_type {                                                 #
// #       "str"   => primary != "str",                                     #
// #       "int"   => matches!(primary, "str" | "bytes"),  …                #
// #   }                                                                    #
// #                                                                        #
// # Splitting the annotation on the `|` CHARACTER is not union            #
// # decomposition: it cuts `dict[str, int] | None` in the wrong place and  #
// # cuts `Literal[\"a|b\"]` inside a string. The five-name whitelist is    #
// # builtin identity by spelling, so `builtins.str` and any aliased import #
// # were "not a known primitive" and silently exempt, while a module's own #
// # `class int` was treated as the numeric tower.                          #
// #                                                                        #
// # Compatibility between two types is `assignable(&TypeNode, &TypeNode)`, #
// # which already exists and already abstains honestly with `None`.        #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn is_factory_incompatible_with_annotation(_factory_type: &str, _field_ann: &str) -> bool {
    panic!(
        "basilisk-checker: `is_factory_incompatible_with_annotation` was DELETED \
         because it decided type compatibility by comparing two RENDERED SPELLINGS \
         against a five-entry builtin-name whitelist, after splitting the annotation \
         on the `|` character to 'strip unions'. It panics because the real \
         implementation — `assignable(&TypeNode, &TypeNode)` on canonical types — DOES \
         NOT EXIST YET at this call site. Do not restore the name match and do not \
         return `false` in its place."
    )
}
