//! Open-document overlays and optimistic preview state.
//!
//! Implements [CONFIGEDITOR-SOURCES-OPEN-BUFFER]. Configuration operations use
//! the exact text and LSP version held by the editor whenever the active source
//! is open; disk remains authoritative only for closed sources.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use basilisk_config::{
    active_config_path, discover_config_document, discover_config_document_with_content,
    ConfigDocument, ConfigDocumentError, ConfigPatch,
};
use tower_lsp::lsp_types::Url;

/// Fully expanded and validated preview retained for apply.
#[derive(Debug, Clone)]
pub(crate) struct PreparedPreview {
    pub(crate) root: PathBuf,
    pub(crate) patch: ConfigPatch,
    pub(crate) disk_revision: String,
}

/// Active configuration plus the version required by a versioned workspace edit.
#[derive(Debug, Clone)]
pub(crate) struct EffectiveConfigDocument {
    pub(crate) document: ConfigDocument,
    pub(crate) version: Option<i32>,
}

#[derive(Debug, Clone)]
struct AppliedDocument {
    disk_revision: String,
    document: ConfigDocument,
}

#[derive(Debug, Clone)]
struct OpenDocument {
    root: PathBuf,
    content: String,
    version: i32,
}

#[derive(Debug, Clone)]
struct PendingOpenEdit {
    base_version: i32,
    document: ConfigDocument,
}

/// Per-server optimistic preview cache and open configuration buffers.
pub(crate) struct ConfigurationEditorState {
    next_preview: AtomicU64,
    previews: Mutex<BTreeMap<String, PreparedPreview>>,
    applied: Mutex<HashMap<PathBuf, AppliedDocument>>,
    open: Mutex<HashMap<PathBuf, OpenDocument>>,
    pending_open_edits: Mutex<HashMap<PathBuf, PendingOpenEdit>>,
}

impl Default for ConfigurationEditorState {
    fn default() -> Self {
        Self {
            next_preview: AtomicU64::new(1),
            previews: Mutex::new(BTreeMap::new()),
            applied: Mutex::new(HashMap::new()),
            open: Mutex::new(HashMap::new()),
            pending_open_edits: Mutex::new(HashMap::new()),
        }
    }
}

impl ConfigurationEditorState {
    /// Whether the URI is a configuration candidate synchronized by clients.
    #[must_use]
    pub(crate) fn is_configuration_candidate(uri: &Url) -> bool {
        uri.to_file_path()
            .ok()
            .is_some_and(|path| is_configuration_path(&path))
    }

    pub(crate) fn insert(&self, preview: PreparedPreview) -> String {
        let id = format!(
            "configuration-preview-{}",
            self.next_preview.fetch_add(1, Ordering::Relaxed)
        );
        let mut previews = lock(&self.previews);
        let _ = previews.insert(id.clone(), preview);
        while previews.len() > 64 {
            let _ = previews.pop_first();
        }
        id
    }

    pub(crate) fn take(&self, id: &str) -> Option<PreparedPreview> {
        lock(&self.previews).remove(id)
    }

    /// Begin tracking a root-level configuration document selected by the client.
    pub(crate) fn open_document(
        &self,
        roots: &[PathBuf],
        uri: &Url,
        content: String,
        version: i32,
    ) -> bool {
        let Ok(path) = uri.to_file_path() else {
            return false;
        };
        let Some(root) = configuration_root(roots, &path) else {
            return false;
        };
        let _ = lock(&self.open).insert(
            path.clone(),
            OpenDocument {
                root,
                content,
                version,
            },
        );
        let _ = lock(&self.pending_open_edits).remove(&path);
        true
    }

    /// Return the current text for a tracked configuration URI.
    pub(crate) fn open_text(&self, uri: &Url) -> Option<String> {
        uri.to_file_path()
            .ok()
            .and_then(|path| lock(&self.open).get(&path).map(|open| open.content.clone()))
    }

    /// Replace a tracked buffer after applying an LSP incremental change batch.
    pub(crate) fn change_document(
        &self,
        uri: &Url,
        content: String,
        version: i32,
    ) -> Option<PathBuf> {
        let path = uri.to_file_path().ok()?;
        let mut open = lock(&self.open);
        let document = open.get_mut(&path)?;
        document.content = content;
        document.version = version;
        let root = document.root.clone();
        let changed_content = document.content.clone();
        drop(open);
        let mut pending = lock(&self.pending_open_edits);
        let reached_pending_edit = pending.get(&path).is_some_and(|edit| {
            version > edit.base_version || changed_content == edit.document.content
        });
        if reached_pending_edit {
            let _ = pending.remove(&path);
        }
        Some(root)
    }

    /// Update optional full text delivered by `didSave` and return its root.
    pub(crate) fn save_document(&self, uri: &Url, content: Option<String>) -> Option<PathBuf> {
        let path = uri.to_file_path().ok()?;
        let mut open = lock(&self.open);
        let document = open.get_mut(&path)?;
        if let Some(content) = content {
            document.content = content;
        }
        Some(document.root.clone())
    }

    /// Stop tracking a configuration buffer and return the root to reload.
    pub(crate) fn close_document(&self, uri: &Url) -> Option<PathBuf> {
        let path = uri.to_file_path().ok()?;
        let removed = lock(&self.open).remove(&path)?;
        let _ = lock(&self.pending_open_edits).remove(&path);
        Some(removed.root)
    }

    pub(crate) fn effective_document(
        &self,
        root: &Path,
    ) -> Result<ConfigDocument, ConfigDocumentError> {
        self.effective_document_with_version(root)
            .map(|effective| effective.document)
    }

    pub(crate) fn effective_document_with_version(
        &self,
        root: &Path,
    ) -> Result<EffectiveConfigDocument, ConfigDocumentError> {
        let path = active_config_path(root);
        let open = lock(&self.open).get(&path).cloned();
        if let Some(open) = open {
            return self.effective_open_document(root, &path, open);
        }
        self.effective_closed_document(root)
            .map(|document| EffectiveConfigDocument {
                document,
                version: None,
            })
    }

    fn effective_open_document(
        &self,
        root: &Path,
        path: &Path,
        open: OpenDocument,
    ) -> Result<EffectiveConfigDocument, ConfigDocumentError> {
        let pending = lock(&self.pending_open_edits).get(path).cloned();
        if let Some(pending) = pending.filter(|pending| pending.base_version >= open.version) {
            return Ok(EffectiveConfigDocument {
                document: pending.document,
                version: Some(open.version.saturating_add(1)),
            });
        }
        let _ = lock(&self.pending_open_edits).remove(path);
        discover_config_document_with_content(root, open.content).map(|document| {
            EffectiveConfigDocument {
                document,
                version: Some(open.version),
            }
        })
    }

    fn effective_closed_document(
        &self,
        root: &Path,
    ) -> Result<ConfigDocument, ConfigDocumentError> {
        let disk = discover_config_document(root)?;
        let mut applied = lock(&self.applied);
        let Some(overlay) = applied.get(root).cloned() else {
            return Ok(disk);
        };
        if disk.revision == overlay.document.revision {
            let _ = applied.remove(root);
            return Ok(disk);
        }
        if disk.revision == overlay.disk_revision {
            return Ok(overlay.document);
        }
        let _ = applied.remove(root);
        Ok(disk)
    }

    pub(crate) fn disk_revision_for(&self, root: &Path, effective_revision: &str) -> String {
        lock(&self.applied)
            .get(root)
            .filter(|overlay| overlay.document.revision == effective_revision)
            .map_or_else(
                || effective_revision.to_owned(),
                |overlay| overlay.disk_revision.clone(),
            )
    }

    pub(crate) fn remember_applied(
        &self,
        root: PathBuf,
        disk_revision: String,
        document: ConfigDocument,
    ) {
        let _ = lock(&self.applied).insert(
            root,
            AppliedDocument {
                disk_revision,
                document,
            },
        );
    }

    /// Bridge the short interval between a successful client edit and didChange.
    pub(crate) fn remember_open_edit(
        &self,
        path: PathBuf,
        base_version: i32,
        document: ConfigDocument,
    ) {
        let _ = lock(&self.pending_open_edits).insert(
            path,
            PendingOpenEdit {
                base_version,
                document,
            },
        );
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn configuration_root(roots: &[PathBuf], path: &Path) -> Option<PathBuf> {
    is_configuration_path(path)
        .then(|| {
            roots
                .iter()
                .find(|root| path.parent() == Some(root.as_path()))
                .cloned()
        })
        .flatten()
}

fn is_configuration_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "pyproject.toml")
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "isolated filesystem fixture failures should abort the test"
)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use basilisk_config::{
        build_rule_patch, ConfigDocument, RuleConfigScope, RuleConfigUpdate, RuleSeverity,
    };
    use tower_lsp::lsp_types::Url;

    use super::ConfigurationEditorState;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "basilisk-config-state-{name}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();
        root
    }

    #[test]
    fn dirty_open_configuration_is_authoritative_over_disk() {
        let root = temp_root("dirty-open");
        let path = root.join("pyproject.toml");
        let uri = Url::from_file_path(&path).unwrap();
        let dirty = concat!(
            "[project]\nname = \"demo\"\n\n",
            "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n",
        );
        let state = ConfigurationEditorState::default();

        assert!(state.open_document(std::slice::from_ref(&root), &uri, dirty.to_owned(), 7));
        let effective = state.effective_document_with_version(&root).unwrap();

        assert_eq!(effective.version, Some(7));
        assert_eq!(effective.document.content, dirty);
        assert_eq!(
            effective.document.config.rules.get("BSK-E0001"),
            Some(&RuleSeverity::Warning)
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "[project]\nname = \"demo\"\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn valid_open_buffer_repairs_malformed_disk_for_editor_operations() {
        let root = temp_root("dirty-repair");
        let path = root.join("pyproject.toml");
        std::fs::write(&path, "[tool.basilisk.rules\n").unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let repaired = "[tool.basilisk.rules]\nBSK-E0001 = \"error\"\n";
        let state = ConfigurationEditorState::default();

        assert!(state.open_document(std::slice::from_ref(&root), &uri, repaired.to_owned(), 11,));
        let effective = state.effective_document_with_version(&root).unwrap();

        assert_eq!(effective.version, Some(11));
        assert_eq!(effective.document.content, repaired);
        assert!(state.close_document(&uri).is_some());
        assert!(state.effective_document(&root).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_versioned_edit_is_visible_without_mutating_incremental_base() {
        let root = temp_root("pending-edit");
        let source_path = root.join("pyproject.toml");
        let uri = Url::from_file_path(&source_path).unwrap();
        let original = std::fs::read_to_string(&source_path).unwrap();
        let state = ConfigurationEditorState::default();
        assert!(state.open_document(std::slice::from_ref(&root), &uri, original.clone(), 7,));
        let document = state.effective_document(&root).unwrap();
        let update = RuleConfigUpdate {
            scope: RuleConfigScope::Project,
            rules: BTreeMap::from([("BSK-E0001".to_owned(), Some(RuleSeverity::Warning))]),
        };
        let rendered = build_rule_patch(&document, &[update]).unwrap();
        let applied = ConfigDocument {
            root: document.root.clone(),
            path: rendered.path.clone(),
            format: document.format,
            exists: true,
            read_only: false,
            shadowed_sources: document.shadowed_sources.clone(),
            content: rendered.content.clone(),
            revision: rendered.revision.clone(),
            config: rendered.config.clone(),
        };

        state.remember_open_edit(source_path, 7, applied);
        let pending = state.effective_document_with_version(&root).unwrap();
        assert_eq!(pending.version, Some(8));
        assert_eq!(pending.document.content, rendered.content);
        assert_eq!(state.open_text(&uri), Some(original));

        let changed_root = state.change_document(&uri, rendered.content.clone(), 8);
        assert_eq!(changed_root, Some(root.clone()));
        let synchronized = state.effective_document_with_version(&root).unwrap();
        assert_eq!(synchronized.version, Some(8));
        assert_eq!(synchronized.document.revision, rendered.revision);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dirty_change_replaces_the_revision_used_by_apply() {
        let root = temp_root("revision-change");
        let path = root.join("pyproject.toml");
        let uri = Url::from_file_path(path).unwrap();
        let state = ConfigurationEditorState::default();
        assert!(state.open_document(
            std::slice::from_ref(&root),
            &uri,
            "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n".to_owned(),
            3,
        ));
        let before = state.effective_document(&root).unwrap().revision;

        let _ = state.change_document(
            &uri,
            "[tool.basilisk.rules]\nBSK-E0001 = \"error\"\n".to_owned(),
            4,
        );
        let after = state.effective_document_with_version(&root).unwrap();

        assert_ne!(after.document.revision, before);
        assert_eq!(after.version, Some(4));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nested_configuration_candidate_is_ignored_instead_of_tracked_as_python() {
        let root = temp_root("nested-candidate");
        let uri = Url::from_file_path(root.join("examples/pyproject.toml")).unwrap();
        let state = ConfigurationEditorState::default();

        assert!(ConfigurationEditorState::is_configuration_candidate(&uri));
        assert!(!state.open_document(
            std::slice::from_ref(&root),
            &uri,
            "[project]\n".to_owned(),
            1,
        ));
        assert!(state.open_text(&uri).is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
