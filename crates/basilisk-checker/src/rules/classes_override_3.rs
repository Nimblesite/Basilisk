//! Implements [`classes_override_3`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `classes_override_3`: `@override` on a method with no matching ancestor method.
//!
//! PEP 698 — a method decorated `@override` (or `typing.override`) must actually
//! override a method declared in a base class. When no ancestor declares a
//! method of that name, the decorator is a lie and the type checker should
//! report it.
//!
//! ```python
//! class Base:
//!     def existing(self) -> int: ...
//!
//! class Child(Base):
//!     @override
//!     def missing(self) -> int:  # E0159: nothing named `missing` in any base
//!         return 1
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "classes_override_3",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override_3",
};

/// Emits `classes_override_3` for `@override` methods that override nothing.
pub(crate) struct OverrideWithoutBaseMethod;

impl Rule for OverrideWithoutBaseMethod {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
