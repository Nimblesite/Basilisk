//! Utility for inserting rule overrides into `pyproject.toml`.

/// Insert or update a rule override in `pyproject.toml` content.
///
/// Adds `[tool.basilisk.rules]` section if missing, then sets `RULE = "severity"`.
pub(super) fn insert_rule_override(content: &str, rule: &str, severity: &str) -> String {
    let section_header = "[tool.basilisk.rules]";
    let entry = format!("{rule} = \"{severity}\"");

    // Check if the rule already exists — update in place.
    if content.contains(section_header) {
        let rule_pattern = format!("{rule} = ");
        if content.contains(&rule_pattern) {
            // Replace existing rule line.
            let mut result = String::with_capacity(content.len());
            for line in content.lines() {
                if line.trim_start().starts_with(&rule_pattern) {
                    result.push_str(&entry);
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            }
            return result;
        }
        // Section exists but rule doesn't — append after section header.
        let mut result = String::with_capacity(content.len() + entry.len() + 2);
        for line in content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim() == section_header {
                result.push_str(&entry);
                result.push('\n');
            }
        }
        return result;
    }

    // No section at all — append at end.
    let mut result = content.to_owned();
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push('\n');
    result.push_str(section_header);
    result.push('\n');
    result.push_str(&entry);
    result.push('\n');
    result
}
