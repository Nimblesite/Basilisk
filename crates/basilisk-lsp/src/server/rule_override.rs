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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_rule_when_no_section_exists() {
        let content = "[tool.basilisk]\nstrict = true\n";
        let result = insert_rule_override(content, "BSK-E0001", "warning");
        assert!(result.contains("[tool.basilisk.rules]"));
        assert!(result.contains("BSK-E0001 = \"warning\""));
    }

    #[test]
    fn insert_rule_when_section_exists_but_rule_absent() {
        let content = "[tool.basilisk.rules]\nBSK-E0002 = \"error\"\n";
        let result = insert_rule_override(content, "BSK-E0001", "warning");
        assert!(result.contains("BSK-E0001 = \"warning\""));
        assert!(result.contains("BSK-E0002 = \"error\""));
    }

    #[test]
    fn update_existing_rule_in_section() {
        let content = "[tool.basilisk.rules]\nBSK-E0001 = \"error\"\n";
        let result = insert_rule_override(content, "BSK-E0001", "warning");
        assert!(result.contains("BSK-E0001 = \"warning\""));
        assert!(!result.contains("BSK-E0001 = \"error\""));
    }

    #[test]
    fn insert_into_empty_content() {
        let result = insert_rule_override("", "BSK-E0001", "off");
        assert!(result.contains("[tool.basilisk.rules]"));
        assert!(result.contains("BSK-E0001 = \"off\""));
    }

    #[test]
    fn insert_when_content_has_no_trailing_newline() {
        let content = "[project]\nname = \"foo\"";
        let result = insert_rule_override(content, "BSK-W0010", "error");
        assert!(result.contains("\n\n[tool.basilisk.rules]\n"));
        assert!(result.contains("BSK-W0010 = \"error\""));
    }
}
