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

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "enums_behaviors",
    docs_url: "https://www.basilisk-python.dev/errors/enums_behaviors",
};

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// Returns `true` when this class is an enum class (directly or transitively).
// ##########################################################################
// # DELETED BODY — `is_enum_class`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # class_map.get(name) + class_or_base_matches over rendered base names.
// #
// # A base class's identity came from its RENDERED NAME, looked up in a map
// # keyed on `ClassInfo::name`. `ClassInfo::bases` is a `Vec<String>` the
// # resolver fills with "simple names only; complex expressions ignored", so:
// #   * a base reached through an alias  ->  MISSED
// #   * a dotted base (`httpx.Client`)   ->  collides with any local class
// #                                          sharing its trailing word
// #   * two classes with one rendered name -> a single map entry
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn is_enum_class(_name: &str, _class_map: &HashMap<&str, &ClassInfo>) -> bool {
    panic!(
        "basilisk-checker: `is_enum_class` was DELETED because it identified base classes by \
         their RENDERED NAMES in a name-keyed map, so an aliased base missed and a \
         dotted base collided with any local class sharing its trailing word. It panics \
         because the real implementation — base expressions resolved through the binding \
         table — DOES NOT EXIST YET. Do not restore the name lookup and do not \
         substitute a default answer in its place."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// Returns `true` when an enum class has any declared members (non-method attributes).
fn has_enum_members(cls: &ClassInfo) -> bool {
    // Enum members are class-body assignments (attributes).
    // Methods are separate (method_names).  An Enum with no attributes has no members.
    // Special attributes like `_value_` are user-defined and count as members.
    // We use a simple heuristic: any annotated or unannotated attribute counts.
    !cls.attributes.is_empty()
}

/// Emits `enums_behaviors` when a class inherits from an Enum that has members.
pub(crate) struct EnumWithMembersFinal;

impl Rule for EnumWithMembersFinal {
    // ##################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // # `class_map.get(base_name.as_str())` decided PEP 435 enum-subclassing by rendered base name.
    // #
    // # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex
    // # expressions ignored") and the lookup map is keyed on
    // # `ClassInfo::name`, so an aliased base MISSED, a dotted base
    // # collided with any local class sharing its trailing word, and two
    // # classes with one rendered name were a single entry.
    // #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
    // ##################################################################
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        panic!(
        "basilisk-checker: `enums_behaviors::check` was DELETED because it identified base classes by \
         their RENDERED NAMES. It panics because the real implementation — base \
         expressions resolved through the binding table — DOES NOT EXIST YET. Do not \
         restore the name lookup and do not substitute a default answer."
    )
    }
}
