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
pub(crate) fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut in_string: Option<char> = None;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '\'' | '"' => in_string = Some(ch),
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(&s[start..idx]);
                    start = idx + 1;
                }
                _ => {}
            },
        }
    }
    parts.push(&s[start..]);
    parts
}
