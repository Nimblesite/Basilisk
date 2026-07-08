//! Implements [CHKARCH-INCREMENTAL-SALSA] test support. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! A salsa database that records query-execution events, shared by every crate
//! that tests the incremental engine.

use std::sync::{Arc, Mutex};

/// A salsa database that records every event's `Debug` string, so tests can
/// assert which derived queries executed (`WillExecute`) versus were reused
/// (`DidValidateMemoizedValue`). Mirrors salsa's own `EventLoggerDatabase`, and
/// implements [`basilisk_db::Db`] so it drives the real Basilisk queries.
#[salsa::db]
#[derive(Clone)]
pub struct EventDb {
    storage: salsa::Storage<Self>,
    logs: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for EventDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDb").finish_non_exhaustive()
    }
}

impl Default for EventDb {
    fn default() -> Self {
        let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&logs);
        let storage = salsa::Storage::new(Some(Box::new(move |event| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(format!("{:?}", event.kind));
            }
        })));
        Self { storage, logs }
    }
}

#[salsa::db]
impl salsa::Database for EventDb {}

#[salsa::db]
impl basilisk_db::Db for EventDb {}

impl EventDb {
    /// Drain the recorded events and return how many were `WillExecute` for the
    /// named query (matched by the query-function name in the event's `Debug`).
    #[must_use]
    pub fn executions_of(&self, query: &str) -> usize {
        let drained: Vec<String> = self
            .logs
            .lock()
            .map(|mut guard| std::mem::take(&mut *guard))
            .unwrap_or_default();
        drained
            .iter()
            .filter(|line| line.starts_with("WillExecute") && line.contains(query))
            .count()
    }
}
