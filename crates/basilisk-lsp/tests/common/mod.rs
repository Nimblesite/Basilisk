//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
//! Shared test utilities for basilisk-lsp integration tests.

use std::process::Child;
use std::time::{Duration, Instant};

/// Maximum time a test Python process is allowed to live (30 seconds).
/// After this, the process is forcefully killed regardless of test outcome.
const MAX_PYTHON_LIFETIME: Duration = Duration::from_secs(30);

/// RAII guard that kills a child process on drop.
///
/// This prevents orphaned Python processes when tests panic or fail before
/// reaching their cleanup code. Every test that spawns a child process
/// MUST use this guard.
pub struct ProcessGuard {
    child: Child,
    spawned_at: Instant,
}

impl ProcessGuard {
    /// Wrap a child process in a guard that kills it on drop.
    #[must_use]
    pub fn new(child: Child) -> Self {
        Self {
            child,
            spawned_at: Instant::now(),
        }
    }

    /// Access the underlying child process mutably (e.g., to read stdout).
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Get the child's PID.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Check if the process has exceeded its maximum lifetime.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.spawned_at.elapsed() > MAX_PYTHON_LIFETIME
    }

    /// Kill the child process explicitly and wait for it.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
