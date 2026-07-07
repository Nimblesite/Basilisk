//! Implements [LSPARCH-ARCH-STACK]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-STACK
//!
//! Analysis-sized stacks for every thread that can run the checker.
//!
//! The resolver and checker walk the AST recursively, and while the parser
//! caps parenthesis and indentation nesting, a long binary-operator chain
//! (`total = 1 + 1 + …` in generated code) yields an arbitrarily deep
//! left-nested `BinOp` tree. On a default ~2 MiB tokio worker stack the
//! workspace scan overflowed and aborted the whole server, crash-looping the
//! editor's restart logic (GitHub #278); the CLI had the same exposure on
//! the process main thread (~8 MiB on macOS/Linux, ~1 MiB on Windows).
//! Every production entry point — LSP and CLI — therefore runs analysis
//! only on threads created here.

use std::io;

/// Stack size for every thread that may run analysis (64 MiB).
///
/// Sized from the measured repro: a 30,000-term chain overflows an 8 MiB
/// stack in release builds (~300 B per level); debug frames are several
/// times larger. 64 MiB comfortably covers generated files hundreds of
/// thousands of terms deep while costing only virtual address space —
/// stacks are committed on demand on every supported platform.
pub const ANALYSIS_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Run `work` to completion on a dedicated thread with an
/// [`ANALYSIS_STACK_SIZE`] stack and return its result.
///
/// Synchronous counterpart of [`block_on_with_analysis_stack`] for entry
/// points that never start a runtime — the CLI's `check`/`fix`/`adopt`
/// dispatch runs the same recursive analyzers directly on its calling
/// thread.
///
/// # Errors
///
/// Returns an `io::Error` if the thread fails to start or panics.
pub fn run_with_analysis_stack<T, F>(thread_name: &str, work: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .name(thread_name.to_owned())
        .stack_size(ANALYSIS_STACK_SIZE)
        .spawn(work)?;
    handle
        .join()
        .map_err(|_panic| io::Error::other("analysis thread panicked"))
}

/// Run `make_future()` to completion on a dedicated thread whose stack —
/// and the stacks of all tokio worker threads created inside it — are
/// [`ANALYSIS_STACK_SIZE`].
///
/// This is the single way production entry points (stdio and WebSocket)
/// start their runtime: tower-lsp polls handler futures both on the
/// `block_on` thread and on runtime workers, so BOTH need analysis-sized
/// stacks.
///
/// # Errors
///
/// Returns an `io::Error` if the thread or the Tokio runtime fails to
/// start, if the future returns an error, or if the runtime thread panics.
pub(crate) fn block_on_with_analysis_stack<F, Fut>(
    thread_name: &str,
    make_future: F,
) -> io::Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = io::Result<()>>,
{
    run_with_analysis_stack(thread_name, move || -> io::Result<()> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(ANALYSIS_STACK_SIZE)
            .build()?;
        rt.block_on(make_future())
    })?
}
