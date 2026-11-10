//! Implements [`dataclasses_slots`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `dataclasses_slots`: Dataclass slots violations.
//!
//! Reports errors when:
//! - a class requests `@dataclass(slots=True)` *and* declares `__slots__`
//!   manually — `slots=True` synthesizes `__slots__`, so the two collide.
//! - `ClassName.__slots__` is accessed on a dataclass that does not define
//!   `__slots__` (neither via `slots=True` nor a manual assignment).
//!
//! ```python
//! @dataclass
//! class DC2:
//!     a: int
//! DC2.__slots__  # E: __slots__ not defined
//! ```
//!
//! # Reduced coverage
//!
//! Two checks have been removed rather than rebuilt. Assignments to undeclared
//! attributes (`self.y = 3` inside a slot-constrained class) were found by
//! slicing the class body out of `module.source`, walking it line by line, and
//! keeping any line whose characters began with the literal prefix `self.`;
//! instance-level access (`DC2().__slots__`) was found by scanning each line for
//! an identifier followed by `(`, counting parentheses by hand, and testing
//! whether the remaining characters began with `.__slots__`.
//!
//! That is scanning Python source for language vocabulary, which the project's
//! first standing rule forbids: recognition is a question about the AST, never
//! about the characters at the use site. Both scanners saw assignments inside
//! strings and comments, missed any statement spanning two lines, and changed
//! verdict under reformatting. They are deleted; those two forms go unreported
//! until they are recovered structurally.

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_slots",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_slots",
};

/// Emits `dataclasses_slots` for dataclass slots violations.
pub(crate) struct DataclassSlotsViolation;

impl Rule for DataclassSlotsViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        check_slots_access_on_non_slots_class(module, diagnostics);
        check_slots_already_defined(module, diagnostics);
    }
}

/// Detect a class that requests `@dataclass(slots=True)` *and* declares
/// `__slots__` manually — `slots=True` synthesizes `__slots__`, so both together
/// is an error (the manual one collides with the generated one).
fn check_slots_already_defined(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    for class in &module.classes {
        if class.is_dataclass && class.is_dataclass_slots && class.has_manual_slots {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`__slots__` is already defined in `{}`: cannot also use \
                     `@dataclass(slots=True)`",
                    class.name
                ),
                class.name_span,
                &module.path,
                Some("Remove the manual `__slots__` assignment or drop `slots=True`".to_owned()),
                Some(
                    "`@dataclass(slots=True)` synthesizes `__slots__`; defining it manually \
                     conflicts"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Detect `ClassName.__slots__` access on a dataclass that does not have slots
/// defined.
fn check_slots_access_on_non_slots_class(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let non_slots_dataclasses: HashSet<&str> = module
        .classes
        .iter()
        .filter(|c| c.is_dataclass && !c.is_dataclass_slots && !c.has_manual_slots)
        .map(|c| c.name.as_str())
        .collect();

    if non_slots_dataclasses.is_empty() {
        return;
    }

    for access in &module.module_attr_accesses {
        if access.attr_name != "__slots__" {
            continue;
        }
        if non_slots_dataclasses.contains(access.object_name.as_str()) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot access `__slots__` on `{}`: class does not define `__slots__`",
                    access.object_name
                ),
                access.span,
                &module.path,
                Some(format!(
                    "Use `@dataclass(slots=True)` or define `__slots__` manually in `{}`",
                    access.object_name
                )),
                Some(
                    "Only classes with `@dataclass(slots=True)` or a manual `__slots__` \
                     definition have a `__slots__` attribute"
                        .to_owned(),
                ),
            ));
        }
    }
}
