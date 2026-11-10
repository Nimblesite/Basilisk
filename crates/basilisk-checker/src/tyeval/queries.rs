//! Implements [TYPEINF-TARGET-TYPELEVEL] — the memoized Salsa queries
//! returning whnf types.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL
//!
//! Two tracked queries put the normalization-by-evaluation engine behind
//! Salsa's memoization, mirroring the definition-level layering of
//! [`crate::incremental_defs`]:
//!
//! - [`type_alias_env`] parses one file and lowers its PEP 695 `type`
//!   statements into an [`AliasEnv`] behind the acceptance conditions
//!   (rejected definitions are left out, so evaluating them projects to
//!   the gradual `Unknown` — never an invented error). The env derives
//!   `PartialEq`, so an edit that leaves the alias set unchanged
//!   **backdates** and downstream memos survive.
//! - [`alias_whnf`] normalizes one alias to weak head normal form. Its
//!   memo is per `(file, alias)`: re-normalization happens only when the
//!   alias environment actually changed.

use basilisk_db::{Db, SourceFile};

use crate::types::InferredType;

use super::eval::Evaluator;
use super::lower::lower_module_aliases;
use super::term::{AliasEnv, TypeTerm};

/// Tracked query: one file's PEP 695 alias environment, lowered and
/// acceptance-checked. Unparseable files produce an empty environment.
#[salsa::tracked(returns(ref))]
pub fn type_alias_env(db: &dyn Db, file: SourceFile) -> AliasEnv {
    let source = file.text(db);
    let mut env = AliasEnv::default();
    let Ok(parsed) = ruff_python_parser::parse_module(source) else {
        return env;
    };
    for lowered in lower_module_aliases(parsed.syntax()) {
        // The acceptance-checked front door: unguarded / non-regular
        // definitions stay out and evaluate gradually to `Unknown`.
        let _ = env.insert(&lowered.name, lowered.def);
    }
    env
}

/// Tracked query: the weak-head-normal-form type of `alias` in `file`,
/// memoized by Salsa per `(file, alias)` on top of the evaluator's own
/// per-application memo. Unknown aliases and truncated evaluations project
/// to the gradual `Unknown` ([TYPEINF-TARGET-GRADUAL]).
#[salsa::tracked(returns(clone))]
pub fn alias_whnf(db: &dyn Db, file: SourceFile, alias: String) -> InferredType {
    let env = type_alias_env(db, file);
    Evaluator::new()
        .evaluate(env, &TypeTerm::Alias(alias, Vec::new()))
        .into_inferred()
}
