//! Span/indent/name utility helpers for BSK-E0149.
//!
//! The geometry helpers (`leading_indent`, `span_for_line`,
//! `line_to_byte_offset`) live in [`crate::rules::shared`]; this module
//! re-exports them so existing call sites continue to work.

pub(super) use crate::rules::shared::{leading_indent, span_for_line};

/// Check whether `name` appears as a whole identifier in `text`.
pub(super) fn contains_name(text: &str, name: &str) -> bool {
    let needle = name.as_bytes();
    let haystack = text.as_bytes();
    let nlen = needle.len();
    if nlen > haystack.len() {
        return false;
    }
    haystack.windows(nlen).enumerate().any(|(idx, window)| {
        if window != needle {
            return false;
        }
        let before_ok = idx == 0
            || haystack
                .get(idx - 1)
                .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
        let after_ok = haystack
            .get(idx + nlen)
            .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
        before_ok && after_ok
    })
}

/// Extract PEP 695 type parameter names from a `[...]` type parameter list
/// on a `class` or `def` line.
///
/// Returns `None` if there is no `[...]` list.
/// Returns `Some(vec)` with the ordered parameter names if found.
pub(super) fn extract_pep695_type_params(line: &str) -> Option<Vec<String>> {
    // Find the opening `[` that is part of a type parameter list.
    // It must appear before `(` (for class), `:` (for def), or `=` (for type).
    let bracket_start = line.find('[')?;
    let colon_or_paren = line.find('(').unwrap_or(line.len());
    let colon_pos = line.find(':').unwrap_or(line.len());
    let eq_pos = line.find('=').unwrap_or(line.len());
    let first_end = colon_or_paren.min(colon_pos).min(eq_pos);

    // The `[` must appear before `(`, `:`, or `=`.
    if bracket_start >= first_end {
        return None;
    }

    // Find the matching `]`.
    let after_bracket = &line[bracket_start + 1..];
    let mut depth = 1usize;
    let mut end_idx = None;
    for (i, ch) in after_bracket.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end_idx = end_idx?;
    let params_text = &after_bracket[..end_idx];

    // Split on commas at depth 0 to get individual parameter specs.
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for ch in params_text.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                let param = current.trim().to_owned();
                if !param.is_empty() {
                    params.push(param);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let last = current.trim().to_owned();
    if !last.is_empty() {
        params.push(last);
    }

    // Extract just the name from each param spec.
    // Specs can be: `T`, `T: Bound`, `**P`, `*Ts`
    let names: Vec<String> = params
        .iter()
        .map(|spec| {
            let s = spec.trim_start_matches('*');
            // Take everything before `:` as the name.
            s.split(':').next().unwrap_or(s).trim().to_owned()
        })
        .filter(|n| !n.is_empty())
        .collect();

    Some(names)
}

/// Extract the bound portion of a type param spec like `T: Sequence[S]`.
/// Returns the text after the `:` if present.
pub(super) fn extract_bound(param_spec: &str) -> Option<&str> {
    let colon_pos = param_spec.find(':')?;
    Some(param_spec[colon_pos + 1..].trim())
}
