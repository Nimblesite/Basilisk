//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_recursive_bases`.
//
// Regression tests for issue #398: `basilisk check` hung on a class that names
// itself among its own bases. `scope/typeddict_meta.rs` `walk_bases()` had only
// a depth guard, so a self-referential class with two bases exploded into a
// 2^64-path DFS instead of terminating.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::common::resolve_src;

/// Ceiling for a resolve that must be effectively instant. Generous so slow CI
/// machines never flake, yet far below the effectively-infinite hang it guards
/// against.
const RESOLVE_DEADLINE: Duration = Duration::from_secs(30);

/// Resolve `src` on a worker thread and fail the test if it does not finish
/// within [`RESOLVE_DEADLINE`] — a hung resolver must fail, not stall CI.
fn resolve_within_deadline(src: &'static str) {
    let (sender, receiver) = mpsc::channel();
    // The handle is deliberately dropped: a hung worker cannot be joined, and
    // the process exiting after the failed test reaps it.
    drop(thread::spawn(move || {
        // Stringify the error: `Box<dyn Error>` is not `Send`, so the raw
        // resolve result cannot cross the channel.
        let outcome = resolve_src(src).map(|_| ()).map_err(|e| e.to_string());
        // The receiver is dropped once the deadline passes; a late send error
        // is then irrelevant because the test has already failed.
        drop(sender.send(outcome));
    }));
    let resolved = receiver
        .recv_timeout(RESOLVE_DEADLINE)
        .unwrap_or_else(|_| panic!("resolver hung on:\n{src}"));
    assert!(resolved.is_ok(), "resolve failed: {:?}", resolved.err());
}

#[test]
fn self_referential_generic_bases_terminate() {
    // Verbatim reproducer from issue #398. Subscripted bases both normalise to
    // "C", so the class ends up its own (doubled) base.
    resolve_within_deadline("class C(C[int], C[bool]):\n    pass\n");
}

#[test]
fn mutually_recursive_bases_terminate() {
    // Two classes forming an inheritance cycle must also resolve promptly.
    resolve_within_deadline(concat!(
        "class A(B):\n",
        "    pass\n",
        "class B(A):\n",
        "    pass\n",
    ));
}
