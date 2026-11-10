//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Enum Checks visitor functions.

// ---------------------------------------------------------------------------
// Float parameter int-only attribute access collection
// ---------------------------------------------------------------------------

/// `int`-only attributes that are invalid to access on a `float`-typed parameter.
///
/// `numerator` and `denominator` are defined on `int` but not on `float`.
pub(super) const INT_ONLY_FLOAT_ATTRS: &[&str] = &["numerator", "denominator"];
