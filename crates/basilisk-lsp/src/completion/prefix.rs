//! Cursor prefix and dot-detection helpers.

/// Extract the identifier fragment immediately before `byte_offset`.
pub(super) fn extract_prefix(text: &str, byte_offset: usize) -> String {
    let before = text.get(..byte_offset.min(text.len())).unwrap_or(text);
    before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Returns `true` when the non-identifier chars just before the cursor end
/// with a `.`.
pub(super) fn is_dot_completion(text: &str, byte_offset: usize) -> bool {
    let before = text.get(..byte_offset.min(text.len())).unwrap_or(text);
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    stripped.ends_with('.')
}

/// Extract the receiver name before the `.` at `byte_offset`, e.g. `"self"`
/// from `"self.<cursor>"`.
pub(super) fn dot_receiver(text: &str, byte_offset: usize) -> Option<String> {
    let before = text.get(..byte_offset.min(text.len())).unwrap_or(text);
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    let before_dot = stripped.strip_suffix('.')?;
    let name: String = before_dot
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}
