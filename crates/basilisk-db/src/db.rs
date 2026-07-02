//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! The Salsa incremental-computation database and its single input query.
//!
//! Basilisk's pipeline is a chain of pure functions — parse, resolve, check —
//! each a function of the file's text. Salsa turns that chain into a
//! demand-driven, memoized query graph: a derived query re-executes only when an
//! input it actually read has changed ([CHKARCH-INCREMENTAL-SALSA]). The
//! **input** is [`SourceFile::text`]; the **derived** queries (parsed AST,
//! resolved module, diagnostics) live in the crates that own that work
//! (`basilisk-parser`, `basilisk-resolver`, `basilisk-checker`), because this
//! crate is the dependency-graph foundation and cannot depend on them.
//!
//! `missing_docs` is expected away for this module alone: salsa's
//! `#[input]`/`#[tracked]`/`#[db]` macros synthesise public constructors,
//! accessors, and setters that cannot carry doc comments. Every hand-written
//! item below is still documented; the expectation is self-cleaning
//! (`unfulfilled_lint_expectations` is denied workspace-wide, so it errors the
//! moment the generated items stop tripping the lint).
#![expect(
    missing_docs,
    reason = "salsa macros generate undocumentable public constructors/accessors/setters"
)]

/// The shared database handle that every Basilisk query accepts.
///
/// Tracked queries in the upstream crates take `&dyn Db`, so any concrete
/// database — the production [`BasiliskDatabase`] or a test double — drives the
/// same query graph.
#[salsa::db]
pub trait Db: salsa::Database {}

/// A single source file: the one **input** query of the engine.
///
/// `text` is the root input. Every derived query (parse → resolve → check)
/// transitively reads it, so mutating it via `set_text` is exactly what makes
/// salsa invalidate and recompute the dependent queries — and *only* those.
/// `path` is a separate input so a rename does not invalidate text-derived work.
#[salsa::input(debug)]
pub struct SourceFile {
    /// Absolute, canonical path — used for diagnostics and import resolution.
    #[returns(ref)]
    pub path: String,
    /// Full UTF-8 source text of the file (the incremental root input).
    #[returns(ref)]
    pub text: String,
}

/// The production storage-backed implementation of [`Db`].
///
/// Cheap to `clone` (the clone shares the same underlying storage and revision
/// counter, per salsa's snapshot semantics) and `Default`-constructed empty.
#[salsa::db]
#[derive(Clone, Default)]
pub struct BasiliskDatabase {
    storage: salsa::Storage<Self>,
}

impl std::fmt::Debug for BasiliskDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Storage holds the entire memoized query graph; summarising keeps the
        // Debug impl O(1) and free of any source text (no PII in logs).
        f.debug_struct("BasiliskDatabase").finish_non_exhaustive()
    }
}

#[salsa::db]
impl salsa::Database for BasiliskDatabase {}

#[salsa::db]
impl Db for BasiliskDatabase {}
