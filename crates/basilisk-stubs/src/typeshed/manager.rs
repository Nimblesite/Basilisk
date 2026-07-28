//! Implements [STUBRES-TYPESHED] one-generation resolution. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED
//!
//! A manager is constructed for one effective, config-free [`TypeshedRequest`].
//! Its first caller performs selection; concurrent CLI/LSP/MCP consumers block
//! on the same [`OnceLock`] and receive the same [`Arc<Snapshot>`]. Success and
//! failure are both memoized for the run/session, so no caller can observe a
//! partially promoted generation. Selection is a local read
//! ([STUBRES-TYPESHED-OFFLINE]) — fast, and never a network wait.

use std::sync::{Arc, OnceLock};

use super::selector::{select_snapshot, SelectionError, SourceBackend};
use super::snapshot::Snapshot;
use super::source::{TypeshedRequest, TypeshedStatus};

/// One config generation's single-flight typeshed manager.
pub struct TypeshedManager {
    request: TypeshedRequest,
    backend: Arc<dyn SourceBackend>,
    selected: OnceLock<Result<Arc<Snapshot>, SelectionError>>,
}

impl std::fmt::Debug for TypeshedManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TypeshedManager")
            .field("request", &self.request)
            .field("ready", &self.selected.get().is_some())
            .finish_non_exhaustive()
    }
}

impl TypeshedManager {
    /// Create a manager for one resolved configuration generation.
    #[must_use]
    pub fn new(request: TypeshedRequest, backend: Arc<dyn SourceBackend>) -> Self {
        Self {
            request,
            backend,
            selected: OnceLock::new(),
        }
    }

    /// The immutable request this manager will resolve exactly once.
    #[must_use]
    pub const fn request(&self) -> &TypeshedRequest {
        &self.request
    }

    /// Resolve or reuse the one complete active snapshot.
    ///
    /// # Errors
    ///
    /// Returns a redacted [`SelectionError`]. The same failure is returned to
    /// every caller for this manager; callers never retry into a different
    /// generation and never receive a substitute source.
    pub fn snapshot(&self) -> Result<Arc<Snapshot>, SelectionError> {
        self.selected
            .get_or_init(|| select_snapshot(&self.request, self.backend.as_ref()).map(Arc::new))
            .clone()
    }

    /// Return the status belonging to the exact active snapshot.
    ///
    /// This is intentionally not a separately synthesized status path: it
    /// first obtains [`Self::snapshot`] and clones that snapshot's status.
    ///
    /// # Errors
    ///
    /// Returns the same redacted selection failure as [`Self::snapshot`].
    pub fn status(&self) -> Result<TypeshedStatus, SelectionError> {
        self.snapshot().map(|snapshot| snapshot.status.clone())
    }

    /// The ready snapshot without starting resolution, if selection already
    /// completed successfully.
    #[must_use]
    pub fn ready_snapshot(&self) -> Option<Arc<Snapshot>> {
        self.selected
            .get()
            .and_then(|result| result.as_ref().ok())
            .map(Arc::clone)
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixtures use fixed embedded assets, threads, and SHA constants"
)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::super::archive::{Archive, ArchiveEntry, ArchiveVfs};
    use super::super::gittree::{FileMode, Oid};
    use super::super::selector::BackendError;
    use super::super::source::{
        LicenseStatus, SourceIdentity, SourceKind, SourceSelection, TypeshedStatus,
    };
    use super::*;

    const PINNED_SHA: &str = "0123456789012345678901234567890123456789";
    const BUNDLE_SHA: &str = "83c2518a9e6abbda0c44592c3483de459198f887";

    struct CountingBackend {
        pinned_calls: AtomicUsize,
        custom_calls: AtomicUsize,
        custom_fails: bool,
    }

    impl CountingBackend {
        fn new(custom_fails: bool) -> Self {
            Self {
                pinned_calls: AtomicUsize::new(0),
                custom_calls: AtomicUsize::new(0),
                custom_fails,
            }
        }
    }

    impl SourceBackend for CountingBackend {
        fn load_custom(&self, _path: &str) -> Result<Snapshot, BackendError> {
            let _ = self.custom_calls.fetch_add(1, Ordering::SeqCst);
            if self.custom_fails {
                return Err(BackendError::Custom);
            }
            let identity = SourceIdentity::Custom {
                digest: "custom-content-digest".to_owned(),
            };
            Ok(fixture_snapshot(identity, SourceKind::Custom))
        }

        fn load_pinned(&self, commit: Oid, explicit: bool) -> Result<Snapshot, BackendError> {
            let _ = self.pinned_calls.fetch_add(1, Ordering::SeqCst);
            // Widen the overlap window: every caller races into `snapshot`, but
            // OnceLock must permit exactly one backend call.
            thread::sleep(Duration::from_millis(10));
            let identity = SourceIdentity::Commit {
                commit,
                pinned: explicit,
            };
            Ok(fixture_snapshot(identity, SourceKind::ExactCommit))
        }

        fn load_bundled(&self) -> Result<Snapshot, BackendError> {
            let commit = Oid::from_hex(BUNDLE_SHA).expect("valid bundle fixture oid");
            Ok(fixture_snapshot(
                SourceIdentity::Bundled { commit },
                SourceKind::Bundled,
            ))
        }
    }

    fn pinned_request() -> TypeshedRequest {
        let commit = Oid::from_hex(PINNED_SHA).expect("valid pinned fixture oid");
        TypeshedRequest {
            selection: SourceSelection::Pinned {
                commit,
                explicit: true,
            },
            store_path: None,
        }
    }

    fn fixture_snapshot(identity: SourceIdentity, active_source: SourceKind) -> Snapshot {
        let commit = identity.commit();
        let archive = Archive::new(vec![
            ArchiveEntry {
                path: "stdlib/VERSIONS".to_owned(),
                mode: FileMode::Regular,
                data: b"os: 3.0-\n".to_vec(),
            },
            ArchiveEntry {
                path: "stdlib/os.pyi".to_owned(),
                mode: FileMode::Regular,
                data: b"name: str\n".to_vec(),
            },
        ]);
        let status = TypeshedStatus {
            active_source,
            commit,
            tree: commit,
            license_status: LicenseStatus::Approved,
            license_reference: None,
            warnings: Vec::new(),
        };
        let uri_identity = identity.uri_component();
        Snapshot::build(
            identity,
            status,
            ArchiveVfs::new(uri_identity, archive),
            None,
        )
        .expect("valid fixture snapshot")
    }

    #[test]
    fn concurrent_callers_share_one_arc_and_one_resolution() {
        let backend = Arc::new(CountingBackend::new(false));
        let manager =
            TypeshedManager::new(pinned_request(), Arc::<CountingBackend>::clone(&backend));
        let snapshots = thread::scope(|scope| {
            let handles: Vec<_> = (0..12)
                .map(|_| scope.spawn(|| manager.snapshot().expect("snapshot")))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("thread"))
                .collect::<Vec<_>>()
        });
        assert_eq!(backend.pinned_calls.load(Ordering::SeqCst), 1);
        let first = snapshots.first().expect("at least one snapshot");
        assert!(snapshots
            .iter()
            .all(|snapshot| Arc::ptr_eq(snapshot, first)));
        assert!(manager.ready_snapshot().is_some());
    }

    #[test]
    fn terminal_failure_is_memoized_without_fallback_retry() {
        let backend = Arc::new(CountingBackend::new(true));
        let manager = TypeshedManager::new(
            TypeshedRequest {
                selection: SourceSelection::Custom {
                    path: "/workspace/custom-typeshed".to_owned(),
                },
                store_path: None,
            },
            Arc::<CountingBackend>::clone(&backend),
        );
        let snapshot_error = manager
            .snapshot()
            .expect_err("a terminal custom-source failure never yields a snapshot");
        assert!(matches!(snapshot_error, SelectionError::Custom(_)));
        let status_error = manager
            .status()
            .expect_err("the memoized failure is reported by status too");
        assert!(matches!(status_error, SelectionError::Custom(_)));
        assert_eq!(backend.custom_calls.load(Ordering::SeqCst), 1);
        assert!(manager.ready_snapshot().is_none());
    }

    #[test]
    fn status_is_cloned_from_the_ready_snapshot() {
        let backend = Arc::new(CountingBackend::new(false));
        let manager = TypeshedManager::new(pinned_request(), backend);
        let snapshot = manager.snapshot().expect("snapshot");
        let status = manager.status().expect("status");
        assert_eq!(status, snapshot.status);
    }
}
