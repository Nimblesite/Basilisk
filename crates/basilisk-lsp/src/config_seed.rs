//! Implements [LSPARCH-CONFIG-SEEDING]. See
//! docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING
//!
//! One-time strict-by-default configuration seeding.
//!
//! When a workspace root's ancestor walk finds no `[tool.basilisk]` table,
//! the LSP writes the strict-by-default seed into the root's
//! `pyproject.toml` (creating the file when absent) BEFORE first analysis —
//! the rule tag plus the binary's bundled typeshed pin, so the workspace is
//! reproducible out of the box ([STUBRES-TYPESHED-WARN]):
//!
//! ```toml
//! [tool.basilisk]
//! typeshed-commit = "<bundled 40-hex sha>"
//!
//! [tool.basilisk.rule-tags]
//! "basilisk" = "error"
//! ```
//!
//! Seeding happens once: the LSP never re-seeds while any `[tool.basilisk]`
//! table exists on the walk, never edits the entry afterwards, and never
//! resurrects anything the user deletes — empty tables are never pruned
//! ([CONFIGEDITOR-SOURCES]), so a table with the entry removed still blocks
//! the seed. The CLI never seeds.

use std::collections::BTreeMap;
use std::path::Path;

use basilisk_config::{
    apply_config_patch, build_configuration_patch, discover_config_document, load_basilisk_config,
    ConfigurationUpdate, RuleConfigUpdate, RuleSeverity, TypeshedConfigKey, TypeshedConfigUpdate,
};

/// Seed `root`'s `pyproject.toml` when the ancestor walk finds no
/// `[tool.basilisk]` table. Returns whether a seed was written.
///
/// Failures are logged and non-fatal: a workspace that cannot be seeded
/// still analyses with the scope defaults (PEP rules at `error`, nothing
/// else runs — [CHKARCH-COMMANDS]).
pub(crate) fn seed_root_if_unconfigured(root: &Path) -> bool {
    let config = load_basilisk_config(root);
    if config.has_config_table() {
        tracing::debug!(
            root = %root.display(),
            "configuration table exists on the ancestor walk; not seeding"
        );
        return false;
    }
    match write_seed(root) {
        Ok(()) => {
            tracing::info!(
                root = %root.display(),
                "seeded strict-by-default configuration (rule-tags basilisk = error, typeshed-commit = bundled)"
            );
            true
        }
        Err(error) => {
            tracing::warn!(
                root = %root.display(),
                %error,
                "failed to seed configuration; continuing with scope defaults"
            );
            false
        }
    }
}

fn write_seed(root: &Path) -> Result<(), basilisk_config::ConfigDocumentError> {
    let document = discover_config_document(root)?;
    let update = ConfigurationUpdate {
        rules: RuleConfigUpdate {
            rules: BTreeMap::new(),
            rule_tags: BTreeMap::from([(
                basilisk_checker::rule_tags::BASILISK.to_owned(),
                Some(RuleSeverity::Error),
            )]),
        },
        // Pin the binary's bundled typeshed commit so the workspace is
        // reproducible from the moment it is opened — never `UNPINNED`
        // ([STUBRES-TYPESHED-WARN]). The bundle is complete inside the
        // binary, so the pin needs no network access and cannot produce a
        // `NO SOURCE` state. The seed only runs when the ancestor walk found
        // no `[tool.basilisk]` table, so no `typeshed-path` can exist for
        // the pin to conflict with ([STUBRES-TYPESHED]).
        typeshed: TypeshedConfigUpdate {
            entries: BTreeMap::from([(
                TypeshedConfigKey::TypeshedCommit,
                Some(basilisk_stubs::typeshed::bundle::bundled_commit_sha().to_owned()),
            )]),
        },
    };
    let patch = build_configuration_patch(&document, &update)?;
    apply_config_patch(&patch)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use std::path::PathBuf;

    use super::seed_root_if_unconfigured;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "basilisk-seed-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// [LSPARCH-CONFIG-SEEDING]: a root with no `[tool.basilisk]` table
    /// anywhere gets the two-line seed, creating `pyproject.toml` if absent.
    #[test]
    fn seeds_a_bare_root_by_creating_pyproject() {
        let root = temp_root("bare");
        assert!(seed_root_if_unconfigured(&root));
        let content = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        assert!(content.contains("[tool.basilisk.rule-tags]"), "{content}");
        assert!(content.contains("basilisk"), "{content}");
        assert!(content.contains("error"), "{content}");
        let config = basilisk_config::load_basilisk_config(&root);
        assert_eq!(
            config.resolve_severity("BSK-0050", &["basilisk"]),
            Some(basilisk_config::RuleSeverity::Error),
            "the seed must turn every basilisk-tagged rule on at error"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// [LSPARCH-CONFIG-SEEDING]: the seed stamps the binary's bundled
    /// typeshed commit alongside the rule tag, so a freshly-opened workspace
    /// is pinned and reproducible — never `UNPINNED`
    /// ([STUBRES-TYPESHED-WARN]) — without any network access (GitHub #343).
    #[test]
    fn seed_stamps_the_bundled_typeshed_commit() {
        let root = temp_root("pin");
        assert!(seed_root_if_unconfigured(&root));
        let content = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        let expected_pin = format!(
            "typeshed-commit = \"{}\"",
            basilisk_stubs::typeshed::bundle::bundled_commit_sha()
        );
        assert!(
            content.contains(&expected_pin),
            "the seed must pin the bundled typeshed commit; got:\n{content}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// [LSPARCH-CONFIG-SEEDING]: seeding happens once — a second
    /// initialization never re-seeds, and deleting the tag entry (leaving the
    /// empty table) never resurrects it.
    #[test]
    fn seeding_is_one_time_and_never_resurrects_deleted_entries() {
        let root = temp_root("once");
        assert!(seed_root_if_unconfigured(&root));
        let seeded = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        assert!(!seed_root_if_unconfigured(&root), "must not re-seed");
        assert_eq!(
            std::fs::read_to_string(root.join("pyproject.toml")).unwrap(),
            seeded,
            "a second initialization must not touch the file"
        );

        // The user deletes the entry; the (never-pruned) empty table remains
        // and blocks any future seed — the house rules stay off.
        std::fs::write(root.join("pyproject.toml"), "[tool.basilisk.rule-tags]\n").unwrap();
        assert!(
            !seed_root_if_unconfigured(&root),
            "an existing (empty) table must block re-seeding"
        );
        let config = basilisk_config::load_basilisk_config(&root);
        assert_eq!(config.resolve_severity("BSK-0050", &["basilisk"]), None);
        let _ = std::fs::remove_dir_all(root);
    }

    /// [LSPARCH-CONFIG-SEEDING]: any `[tool.basilisk]` table on the ancestor
    /// walk — including a parent directory's — blocks the seed.
    #[test]
    fn ancestor_table_blocks_seeding() {
        let root = temp_root("ancestor");
        let child = root.join("pkg");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"warning\"\n",
        )
        .unwrap();
        assert!(!seed_root_if_unconfigured(&child));
        assert!(
            !child.join("pyproject.toml").exists(),
            "no file may be created under a configured ancestor"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// [LSPARCH-CONFIG-SEEDING]: seeding preserves unrelated pyproject
    /// content when the file already exists without a `[tool.basilisk]` table.
    #[test]
    fn seeding_preserves_existing_pyproject_content() {
        let root = temp_root("preserve");
        std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();
        assert!(seed_root_if_unconfigured(&root));
        let content = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        assert!(content.contains("[project]"), "{content}");
        assert!(content.contains("name = \"demo\""), "{content}");
        assert!(content.contains("[tool.basilisk.rule-tags]"), "{content}");
        let _ = std::fs::remove_dir_all(root);
    }
}
