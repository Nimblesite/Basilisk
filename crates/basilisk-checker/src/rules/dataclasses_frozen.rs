//! Implements [`dataclasses_frozen`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `dataclasses_frozen`: Assignment to attribute of a frozen dataclass instance, or invalid
//! frozen/non-frozen dataclass inheritance.
//!
//! `@dataclass(frozen=True)` instances are immutable — their attributes cannot
//! be reassigned after construction.  Additionally, a frozen dataclass cannot
//! inherit from a non-frozen one, and vice versa.
//!
//! ```python
//! @dataclass(frozen=True)
//! class Point:
//!     x: float
//!
//! p = Point(1.0)
//! p.x = 2.0  # E: dataclass is frozen
//!
//! @dataclass          # E: non-frozen cannot inherit from frozen
//! class Sub(Point):
//!     pass
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

/// ORPHANED BY A DELETION, NOT UNUSED. Both emitters — `check_inheritance`
/// and `check_frozen_instance_assigns` — were deleted for deciding frozen
/// dataclass identity by class SPELLING. The code and its docs URL are the
/// rule's published identity and are what the rebuild emits under.
#[expect(
    dead_code,
    reason = "both emitters were deleted for joining classes by spelling; the rule's \
              published error code is retained for the identity-based rebuild"
)]
const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_frozen",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_frozen",
};

/// Emits `dataclasses_frozen` for:
/// - Attribute assignments on frozen dataclass instances at module level.
/// - Dataclass inheritance where frozen/non-frozen status is mixed.
pub(crate) struct FrozenDataclassAssignment;

impl Rule for FrozenDataclassAssignment {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let transform_classes = super::guards::collect_transform_classes(module);

        // This map is keyed on `ClassInfo::name` — a RENDERED SPELLING. It is
        // built here only so the deleted `check_inheritance` call site stays
        // visible as the rebuild map; the callee panics before reading it.
        let class_frozen: HashMap<&str, (bool, bool)> = module
            .classes
            .iter()
            .map(|c| {
                let is_dc = c.is_dataclass || transform_classes.contains_key(&c.name_span);
                let is_frozen = c.is_dataclass_frozen
                    || transform_classes
                        .get(&c.name_span)
                        .is_some_and(|info| info.frozen);
                (c.name.as_str(), (is_dc, is_frozen))
            })
            .collect();

        check_inheritance(&class_frozen, module, diagnostics);
        check_frozen_instance_assigns(module, diagnostics);
    }
}

// ##########################################################################
// # DELETED BODY — `dataclasses_frozen::check_inheritance`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `class_frozen.get(base_name.as_str())` decided frozen/non-frozen dataclass inheritance by rendered base name.
// #
// # `ClassInfo::bases` is a `Vec<String>` the resolver fills with "simple
// # names only; complex expressions ignored", and the lookup map is keyed on
// # `ClassInfo::name`. So a base reached through an alias MISSED, a dotted
// # base collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn check_inheritance(
    _class_frozen: &HashMap<&str, (bool, bool)>,
    _module: &ResolvedModule,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `dataclasses_frozen::check_inheritance` was DELETED because it identified base classes by \
         their RENDERED NAMES, so an aliased base missed and a dotted base collided with \
         any local class sharing its trailing word. It panics because the real \
         implementation — base expressions resolved through the binding table — DOES NOT \
         EXIST YET. Do not restore the name lookup and do not substitute a default \
         answer in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `check_frozen_instance_assigns`. DO NOT RESTORE IT AND   #
// # DO NOT RETURN WITHOUT DIAGNOSING.                                       #
// #                                                                         #
// #   let mut frozen_classes: HashSet<&str> =                               #
// #       collect_name_set_where(&module.classes, |c| c.is_dataclass_frozen)#
// #   frozen_classes.insert(class.name.as_str());                           #
// #   let callee = constructed_class_name(slice_span(source, rhs_span)?);   #
// #   if frozen_classes.contains(callee) { .. }                             #
// #   instance_class.get(assign.object_name.as_str())                       #
// #                                                                         #
// # THREE SPELLING JOINS IN A ROW, AND THE VERDICT COMES OUT THE END.       #
// # `frozen_classes` is a set of CLASS NAMES. The variable's class is read  #
// # out of RAW SOURCE. The two are matched by characters, and an attribute  #
// # assignment is then reported as an error against that match. So:         #
// #                                                                         #
// #   * `import models; c = models.Config()` matched a LOCAL frozen         #
// #     `Config` and reported correct code as an error;                     #
// #   * `Alias = Config; c = Alias()` matched nothing and every mutation of #
// #     a genuinely frozen instance went unreported;                        #
// #   * `collect_transform_classes` resolves PEP 681 frozen-ness on         #
// #     DEFINITION SITES, correctly — and the loop above threw that         #
// #     identity away to insert `class.name.as_str()` into the spelling set.#
// #                                                                         #
// # Which class a variable holds is the resolved type of the call's `func`, #
// # and which class an attribute assignment targets is the resolved type of #
// # its object expression. Neither is a string. The rebuild takes both from #
// # the binding table and compares `ClassInfo::name_span` to the frozen     #
// # set, which becomes `HashSet<Span>`.                                     #
// #                                                                         #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                       #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn check_frozen_instance_assigns(_module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
    panic!(
        "basilisk-checker: `dataclasses_frozen::check_frozen_instance_assigns` was DELETED \
         because it built a set of frozen CLASS NAMES, read each variable's constructed \
         class out of RAW SOURCE, and matched the two by characters — so an imported \
         `models.Config()` was reported against a local frozen `Config`, and an aliased \
         construction of a genuinely frozen class was never reported at all. It panics \
         because the real implementation — the call's `func` and the assignment's object \
         resolved through the binding table to definition sites — DOES NOT EXIST YET. Do \
         not restore the name set and do not return without diagnosing in its place."
    )
}

// ##########################################################################
// # DELETED BODY — the constructed-class reduction. DO NOT RESTORE IT AND  #
// # DO NOT RETURN `""` IN ITS PLACE.                                       #
// #                                                                        #
// #   let callee = rhs_text.split(['(', '[']).next().unwrap_or("").trim(); #
// #   let callee = callee.rsplit('.').next().unwrap_or(callee);            #
// #                                                                        #
// # It read the constructed class out of RAW SOURCE by cutting at the      #
// # first `(` or `[`, then discarded the qualifier. `models.Config()` and  #
// # a local `Config()` became the same class, so a frozen `Config` defined #
// # in this module made assignments to an INSTANCE OF A DIFFERENT CLASS    #
// # an error — a diagnostic on correct code, produced by a name collision. #
// #                                                                        #
// # Which class a variable holds is the type of `ExprCall::func` resolved  #
// # through the binding table.                                             #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
#[expect(
    dead_code,
    reason = "its only caller was itself deleted for joining classes by spelling; this shell \
              stays as the map of what the identity-based rebuild must replace"
)]
fn constructed_class_name(_rhs_text: &str) -> &str {
    panic!(
        "basilisk-checker: the constructed-class reduction in `dataclasses_frozen` was \
         DELETED because it read the class out of RAW SOURCE by cutting at the first \
         `(` or `[` and then discarded the module qualifier with `rsplit('.')`, so a \
         local frozen class hijacked every same-named class from anywhere else. It \
         panics because the real implementation — resolving the callee through the \
         binding table — DOES NOT EXIST YET. Do not restore the splitting and do not \
         return `\"\"` in its place."
    )
}
