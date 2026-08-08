//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! Source-text geometry, top-level splitting, and line tokenisation shared by
//! rules that still scan annotation or source text ([CHKARCH-DIAG]). Text
//! scanning is not a type mechanism: types come from the engine
//! ([TYPEINF-ALGO]). No new code may call into this module — it is deleted
//! outright per [NARROWPLAN-INTEGRATION] when its last consumer migrates.

/// Split `s` at every top-level comma, respecting bracket nesting and string
/// literals — a comma inside quotes (`Literal[',']`) is part of the literal
/// value, not a separator (issue #316).
///
/// Returns slices into the original string (no allocation for the parts
/// themselves). Callers that need trimmed/owned values can chain
/// `.iter().map(|p| p.trim().to_owned())`.
