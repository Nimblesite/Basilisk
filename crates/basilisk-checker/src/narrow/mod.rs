//! Implements [TYPEINF-NARROWING] and [TYPEINF-TARGET-NARROWING]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! The Stage 2 flow-sensitive narrowing engine
//! ([NARROWPLAN-CHECKLIST], docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md):
//!
//! - [`set_ops`] — occurrence-typing set operations over
//!   [`crate::types::InferredType`]: `intersect` (narrow to a guard type) and
//!   `subtract` (the complement branch), the intersection-and-negation model;
//! - [`env`] — the scoped narrowing environment: branch push/pop, complement
//!   application, join at merges, and nested-function boundaries;
//! - [`guards`] — consumption of the resolver's collected
//!   [`basilisk_resolver::scope::narrowing_types::NarrowingGuard`]s
//!   (`isinstance`, `is None`, truthiness, `TypeGuard`, `TypeIs`, `assert`,
//!   `match`) into positive/negative environment updates;
//! - [`reachability`] — inference-driven divergence (a `Never`-typed call
//!   statement diverges; `while True:` without `break` diverges) replacing
//!   the pattern-matched last-statement idiom;
//! - [`rebind`] — the binding collector behind narrow invalidation: a
//!   rebound name never keeps a narrow proven for its previous value.

pub mod env;
pub mod flow;
pub mod guards;
mod reachability;
mod rebind;
pub mod set_ops;

pub use env::NarrowEnv;
pub use flow::{analyse_function, analyse_function_in, FlowResult, NarrowedUse};
pub(crate) use reachability::{stmt_diverges, SynthFn};
pub(crate) use rebind::{bound_names, target_names};
pub use guards::{guard_outcomes, guard_outcomes_in, GuardOutcome, NarrowContext, TypedDictKeys};
pub use set_ops::{intersect, subtract};
