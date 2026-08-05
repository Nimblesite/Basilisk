//! Implements [TYPEINF-TARGET] and [TYPEINF-TARGET-TYPELEVEL] — Stage 3
//! type-level evaluation. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST
//! ("Stage 3 — type-level evaluation groundwork") and
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL.
//!
//! Python's type-hint sublanguage is Turing-complete (Roth,
//! <https://arxiv.org/abs/2208.14755>), so recursive/parameterised type
//! aliases must be *evaluated*, not expanded eagerly. This module is the
//! normalization-by-evaluation engine:
//!
//! - [`term`] — the term language: ground types, constructors, alias
//!   applications, **kind `Type → Type` operator values** ([`Kind`],
//!   [`TypeTerm::Op`]/[`TypeTerm::Apply`] — the mapped-type
//!   representation), and **conditional types** as assignability-guarded
//!   rewrites ([`CondTerm`]); plus the [`AliasEnv`] with its
//!   acceptance-checked front door and the opt-in
//!   [`AliasEnv::insert_undecidable`] escape hatch;
//! - [`accept`] — the GHC-style (Paterson/Coverage-analogue) acceptance
//!   conditions: guardedness (contractivity) and regularity
//!   (non-growing self-applications), producing an [`Acceptance`] verdict
//!   consumed by both the engine and the `generics_syntax_scoping` rule;
//! - [`eval`] — lazy (call-by-need) unfolding to **weak head normal
//!   form** with **fuel/depth bounds**, **memoization** per application,
//!   union distribution for conditionals, and the **`Divergent`
//!   fallback** projecting to the gradual `Unknown` — truncation NEVER
//!   invents an error ([TYPEINF-TARGET-GRADUAL]);
//! - [`lower`] — total, gradual lowering from Ruff AST `type`-statement
//!   expressions (string forward references included) into terms;
//! - [`queries`] — the memoized Salsa layer: [`type_alias_env`] (lowered,
//!   acceptance-checked, backdating) and [`alias_whnf`] per
//!   `(file, alias)`.

pub mod accept;
pub mod eval;
pub mod lower;
pub mod queries;
pub mod term;

pub use accept::{classify, Acceptance};
pub use eval::{Eval, Evaluator};
pub use lower::{lower_module_aliases, LowerCtx, LoweredAlias};
pub use queries::{alias_whnf, type_alias_env};
pub use term::{AliasDef, AliasEnv, CondTerm, Kind, TypeTerm};
