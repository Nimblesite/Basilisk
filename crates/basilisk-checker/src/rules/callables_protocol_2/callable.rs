//! Implements [`callables_protocol_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Type compatibility helpers for `callables_protocol_2`.

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Returns `true` when `source` is assignable to `target` for the purposes of
/// callable/protocol parameter checking.
pub(super) fn types_compat(target: &str, source: &str) -> bool {
    if target == source {
        return true;
    }
    if target.is_empty() || source.is_empty() {
        return true;
    }
    if target == "int" && source == "float" {
        return true;
    }
    if target == "float" && source == "int" {
        return true;
    }
    if target == "bool" && source == "int" {
        return true;
    }
    if target.contains(" | ") {
        return target.split(" | ").any(|m| m.trim() == source);
    }
    let builtins = [
        "int", "str", "float", "bool", "bytes", "None", "complex", "object",
    ];
    if builtins.contains(&target) && builtins.contains(&source) {
        return false;
    }
    true
}
