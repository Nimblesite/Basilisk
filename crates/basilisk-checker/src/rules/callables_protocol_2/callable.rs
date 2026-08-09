//! Implements [`callables_protocol_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Type compatibility helpers for `callables_protocol_2`.

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Returns `true` when `source` is assignable to `target` for the purposes of
/// callable/protocol parameter checking.
pub(super) fn types_compat(_target: &str, _source: &str) -> bool {
    // The former implementation compared rendered annotation strings, split
    // unions on textual ` | `, and hard-coded builtin spellings. That was
    // illegal and has been deleted. This panic is mandatory until callable
    // compatibility is implemented from resolved types and symbol identity.
    panic!("callables_protocol_2::types_compat has no legal resolved-type implementation");
}
