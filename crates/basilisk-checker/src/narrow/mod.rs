//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
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
//!   `match`) into positive/negative environment updates.

pub mod env;
pub mod flow;
pub mod guards;
pub mod set_ops;

pub use env::NarrowEnv;
pub use flow::{analyse_function, FlowResult, NarrowedUse};
pub use guards::{guard_outcomes, GuardOutcome};
pub use set_ops::{intersect, subtract};
