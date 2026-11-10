//! Implements [`enums_behaviors`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `enums_behaviors`: Invalid Enum subclassing.
//!
//! An Enum class with one or more defined members is implicitly final and
//! cannot be subclassed. Only Enum subclasses with no members can be used
//! as bases for other Enum classes.
//!
//! ```python
//! class Color(Enum):
//!     RED = 1
//!     GREEN = 2
//!
//! class ExtendedColor(Color):  # E — Color has members and is implicitly final
//!     BLUE = 3
//! ```

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "enums_behaviors",
    docs_url: "https://www.basilisk-python.dev/errors/enums_behaviors",
};

// ##########################################################################
// # `is_enum_class` IS GONE, NOT REBUILT IN PLACE.                         #
// #                                                                       #
// # Its body was `class_map.get(name)` plus `class_or_base_matches` over   #
// # RENDERED base names, so a base reached through an alias MISSED, a      #
// # dotted base (`httpx.Client`) collided with any local class sharing its #
// # trailing word, and two classes with one rendered name were a single    #
// # map entry.                                                             #
// #                                                                       #
// # `ClassInfo::is_enum` already answers "is this class an `Enum`,         #
// # directly or transitively?" — its direct bases are classified by        #
// # binding resolution at collection time, and `visitor::propagate_enum_   #
// # bases` closes the relation over `ClassGraph`'s definition-site-keyed   #
// # edges. There is no name lookup left to perform.                        #
// #                                                                       #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs             #
// ##########################################################################

/// Returns `true` when an enum class has any declared members (non-method
/// attributes).
///
/// NOT PEP 435 MEMBERSHIP. This counts every entry in `ClassInfo::attributes`,
/// which is a superset of the enum's members, so it answers `true` for classes
/// that have no members at all:
///
/// * an ANNOTATION-ONLY attribute (`weight: int`) declares a type, not a
///   member — <https://docs.python.org/3/library/enum.html#supported-sunder-names>;
/// * a descriptor or `property` in the class body is not a member;
/// * `enum.nonmember(...)` exists precisely to opt a value OUT of membership,
///   and is counted here anyway;
/// * `_ignore_` names are excluded at runtime and not here.
///
/// The class-identity half of this rule was rebuilt on definition sites; the
/// MEMBERSHIP half was not, and this is it. Real membership needs the resolved
/// value of each class-body assignment. Until that exists, this over-counts,
/// which makes the rule fire on some enums that have no members.
fn has_enum_members(cls: &ClassInfo) -> bool {
    !cls.attributes.is_empty()
}

/// Emits `enums_behaviors` when a class inherits from an Enum that has members.
pub(crate) struct EnumWithMembersFinal;

// ##################################################################
// # REBUILT ON DEFINITION-SITE IDENTITY.                           #
// #                                                                #
// # The deleted body used `class_map.get(base_name.as_str())` to   #
// # decide PEP 435 enum-subclassing by RENDERED base name.         #
// #                                                                #
// # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex       #
// # expressions ignored") and the lookup map was keyed on          #
// # `ClassInfo::name`, so an aliased base MISSED, a dotted base    #
// # collided with any local class sharing its trailing word, and   #
// # two classes with one rendered name were a single entry.        #
// #                                                                #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs     #
// ##################################################################
impl Rule for EnumWithMembersFinal {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let graph = basilisk_resolver::ClassGraph::new(&module.classes);

        for class in &module.classes {
            for base in &class.resolved_bases {
                let basilisk_resolver::ResolvedBase::LocalClass(site) = base.resolved else {
                    // A base from another module: this module cannot see its
                    // members ([CHKARCH-CONFORMANCE-MODE]).
                    continue;
                };
                let Some(base_class) = graph.at(site) else {
                    continue;
                };
                // `is_enum` is the transitive answer, closed over resolved
                // bases by the resolver.
                if !base_class.is_enum || !has_enum_members(base_class) {
                    continue;
                }
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{}` cannot subclass `{}`: an enum with members is implicitly final",
                        class.name, base_class.name
                    ),
                    base.span,
                    &module.path,
                    Some(format!(
                        "Move the members of `{}` into a separate subclass, or subclass the \
                         member-free base instead",
                        base_class.name
                    )),
                    Some(
                        "PEP 435: an enumeration that defines any member cannot be \
                         subclassed"
                            .to_owned(),
                    ),
                ));
                break;
            }
        }
    }
}
