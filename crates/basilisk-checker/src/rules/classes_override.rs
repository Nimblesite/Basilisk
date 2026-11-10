//! Implements [`classes_override`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `classes_override`: Incompatible method override.
//!
//! When a class method marked with `@override` has a different parameter
//! signature or return type than the corresponding method in a same-module
//! base class, Basilisk reports an incompatible override.
//!
//! ```python
//! class Base:
//!     def process(self: Base, data: str) -> str: ...
//!
//! class Child(Base):
//!     @override
//!     def process(self: Child, data: int) -> int: ...  # E0016
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "classes_override",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override",
};

/// Emits `classes_override` for `@override` methods with incompatible signatures.
pub(crate) struct IncompatibleOverride;

impl Rule for IncompatibleOverride {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
