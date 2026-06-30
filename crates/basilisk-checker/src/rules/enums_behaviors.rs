//! Implements [`enums_behaviors`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-immutability
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "enums_behaviors",
    docs_url: "https://www.basilisk-python.dev/errors/enums_behaviors",
};

/// Returns `true` when this class is an enum class (directly or transitively).
fn is_enum_class(name: &str, class_map: &HashMap<&str, &ClassInfo>) -> bool {
    let Some(cls) = class_map.get(name) else {
        return false;
    };
    if cls.is_enum {
        return true;
    }
    cls.bases
        .iter()
        .any(|b| is_enum_class(b.as_str(), class_map))
}

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
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_map = super::shared::class_name_map(&module.classes);

        for child in &module.classes {
            for base_name in &child.bases {
                // Only care about bases defined in this module.
                let Some(base_cls) = class_map.get(base_name.as_str()) else {
                    continue;
                };

                // The base must be an enum class.
                if !is_enum_class(base_name.as_str(), &class_map) {
                    continue;
                }

                // The base must have members.
                if !has_enum_members(base_cls) {
                    continue;
                }

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Cannot subclass `{base_name}` because it has members and is implicitly final"
                    ),
                    child.name_span,
                    &module.path,
                    Some(
                        "Enum classes with members cannot be subclassed. \
                         Only memberless Enum bases can be extended."
                            .to_owned(),
                    ),
                    Some(
                        "PEP 435: An Enum class with one or more members cannot be subclassed"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
