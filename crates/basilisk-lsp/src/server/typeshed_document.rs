//! Implements [STUBRES-TYPESHED] read-only archive navigation.
//!
//! Definition locations inside the immutable Typeshed VFS use stable
//! `typeshed:<identity>/<entry>` URIs. This request returns the exact bytes from
//! the active snapshot so clients can open them without treating the URI as a
//! filesystem path.

use basilisk_stubs::typeshed::snapshot::Snapshot;
use serde::{Deserialize, Serialize};
use tower_lsp::jsonrpc::Result as LspResult;

use super::LspServer;

/// Parameters for `basilisk/typeshedDocument`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeshedDocumentParams {
    /// Stable logical URI returned by a Typeshed definition location.
    pub uri: String,
}

/// Immutable virtual document returned to an editor content provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeshedDocument {
    /// Exact active-snapshot text.
    pub text: String,
    /// Editor language identifier.
    pub language_id: String,
    /// Always true: archive documents cannot be modified in place.
    pub read_only: bool,
}

impl LspServer {
    /// Return one document from the active immutable Typeshed snapshot.
    pub(crate) async fn typeshed_document(
        &self,
        params: TypeshedDocumentParams,
    ) -> LspResult<Option<TypeshedDocument>> {
        let generations = self.typeshed_generations.read().await;
        Ok(generations.values().find_map(|generation| {
            generation
                .ready_snapshot()
                .and_then(|snapshot| document_for_snapshot(snapshot, &params.uri))
        }))
    }
}

fn document_for_snapshot(snapshot: &Snapshot, uri: &str) -> Option<TypeshedDocument> {
    let prefix = format!("typeshed:{}/", snapshot.identity.uri_component());
    let entry = uri.strip_prefix(&prefix)?;
    let text = snapshot.vfs.read_str(entry)?;
    Some(TypeshedDocument {
        text: text.to_owned(),
        language_id: if std::path::Path::new(entry)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pyi"))
        {
            "python".to_owned()
        } else {
            "plaintext".to_owned()
        },
        read_only: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_document_returns_exact_active_identity_bytes() {
        let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot();
        assert!(
            snapshot.is_ok(),
            "bundled snapshot must activate: {snapshot:?}"
        );
        let Ok(snapshot) = snapshot else {
            return;
        };
        let uri = format!(
            "typeshed:{}/stdlib/builtins.pyi",
            snapshot.identity.uri_component()
        );
        let document = document_for_snapshot(&snapshot, &uri);
        assert!(document.is_some());
        let Some(document) = document else {
            return;
        };
        assert!(document.text.contains("class str"));
        assert_eq!(document.language_id, "python");
        assert!(document.read_only);
    }

    #[test]
    fn another_identity_cannot_read_the_active_snapshot() {
        let Ok(snapshot) = basilisk_stubs::typeshed::bundle::bundled_snapshot() else {
            return;
        };
        assert!(
            document_for_snapshot(&snapshot, "typeshed:another-identity/stdlib/builtins.pyi")
                .is_none()
        );
    }
}
