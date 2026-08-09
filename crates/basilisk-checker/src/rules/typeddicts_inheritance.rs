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

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this diagnostic constructor is \
              correct and is retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_inheritance",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_inheritance",
};

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this diagnostic constructor is \
              correct and is retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
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

// ##########################################################################
// # DELETED BODY — `check_mixed_bases`. DO NOT RESTORE IT AND DO NOT       #
// # REPLACE IT WITH A PLACEHOLDER THAT RETURNS WITHOUT CHECKING.           #
// #                                                                        #
// # Two spelling dependencies, either one fatal:                           #
// #                                                                        #
// #   const EXEMPT: &[&str] = &["object"];                                 #
// #   if EXEMPT.contains(&base.as_str()) { continue; }                     #
// #                                                                        #
// # recognised the top type by its builtin SPELLING, so a module defining  #
// # its own `class object: ...` had a genuine non-TypedDict base silently  #
// # exempted, while `builtins.object` reached under any other name was not #
// # exempted at all. And:                                                  #
// #                                                                        #
// #   is_transitive_typeddict(base.as_str(), class_map)                    #
// #                                                                        #
// # identified every base by RENDERED NAME through a name-keyed map (that  #
// # helper is itself DELETED — see basilisk-resolver/src/scope/            #
// # typeddict_meta.rs).                                                    #
// #                                                                        #
// # The replacement resolves each base EXPRESSION through the binding      #
// # table: the top type is `TypingForm::ObjectClass`, and TypedDict-ness   #
// # follows resolved base classes rather than spellings.                   #
// #                                                                        #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs             #
// ##########################################################################

/// DELETED — panics. The signature survives only so its caller stays visible
/// as the rebuild map; see the banner above.
fn check_mixed_bases(
    _cls: &ClassInfo,
    _class_map: &HashMap<&str, &ClassInfo>,
    _path: &str,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `typeddicts_inheritance::check_mixed_bases` was DELETED \
         because it exempted the top type by matching the SPELLING \"object\" and \
         identified TypedDict bases by rendered name. It panics because the real \
         implementation — each base resolved through the binding table, with the top \
         type recognised as `TypingForm::ObjectClass` — DOES NOT EXIST YET. Do not \
         restore the spelling list and do not skip the check in its place."
    )
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
        let class_map = super::shared::class_name_map(&module.classes);

        for cls in &module.classes {
            if !basilisk_resolver::is_transitive_typeddict(cls.name.as_str(), &class_map) {
                continue;
            }

            check_mixed_bases(cls, &class_map, &module.path, diagnostics);
        }
    }
}
