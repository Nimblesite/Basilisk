//! Shared fixtures for the editor persistence suites ([CONFIGEDITOR-SOURCES]).
//!
//! One definition of a throwaway root, a validated document, and the rule/tag
//! updates, so the rule, Typeshed, and caching suites cannot drift apart about
//! what they are patching.

use std::collections::BTreeMap;
use std::path::Path;

use super::super::{
    content_revision, discover_config_document_with_content, ConfigDocument, ConfigPatch,
    RuleConfigUpdate,
};
use crate::{BasiliskConfig, RuleSeverity};

pub(super) fn temp_root(unique: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bsk_editor_{unique}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A document carrying unrelated project content around an empty
/// `[tool.basilisk]` table — a comment and a neighbouring `[project]` table.
pub(super) const PROJECT_DOCUMENT: &str = "# keep\n[project]\nname = \"demo\"\n\n[tool.basilisk]\n";

/// Assert a rendered patch left every unrelated byte of [`PROJECT_DOCUMENT`]
/// alone ([CONFIGEDITOR-SOURCES]). Both setting panels' writers owe this, so
/// they assert it through one routine rather than two copies of it.
pub(super) fn assert_project_content_preserved(content: &str) {
    assert!(content.contains("# keep"), "comments must survive a patch");
    assert!(
        content.contains("name = \"demo\""),
        "a neighbouring table must survive a patch"
    );
}

pub(super) fn document_for(root: &Path, content: &str) -> ConfigDocument {
    discover_config_document_with_content(root, content.to_owned()).unwrap()
}

pub(super) fn set_rule(code: &str, severity: RuleSeverity) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::from([(code.to_owned(), Some(severity))]),
        rule_tags: BTreeMap::new(),
    }
}

pub(super) fn remove_rule(code: &str) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::from([(code.to_owned(), None)]),
        rule_tags: BTreeMap::new(),
    }
}

pub(super) fn set_tag(tag: &str, severity: RuleSeverity) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::new(),
        rule_tags: BTreeMap::from([(tag.to_owned(), Some(severity))]),
    }
}

/// A document the editor holds in a state discovery would have rejected
/// (stale buffer, external corruption) — the patch layer re-validates rather
/// than trusting the caller.
pub(super) fn manual_document(root: &Path, content: &str, read_only: bool) -> ConfigDocument {
    ConfigDocument {
        root: root.to_path_buf(),
        path: root.join("pyproject.toml"),
        exists: true,
        read_only,
        content: content.to_owned(),
        revision: content_revision(content),
        config: BasiliskConfig::default(),
    }
}

pub(super) fn manual_patch(path: std::path::PathBuf, base: &str, content: &str) -> ConfigPatch {
    ConfigPatch {
        path,
        base_revision: content_revision(base),
        revision: content_revision(content),
        content: content.to_owned(),
        config: BasiliskConfig::default(),
    }
}
