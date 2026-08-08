//! Implements [RESOLV-CANONICAL].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! Canonical symbol resolution — how Basilisk recognises specification forms.
//!
//! Recognition is a question about DEFINITIONS, not about characters. A use
//! site is resolved through the module's imports and bindings to the
//! definition it refers to ([`binding`]), and that definition site is looked up
//! in the specification registry ([`form`]).

mod binding;
mod form;
mod registry;
mod relation;
mod type_node;

pub use binding::{BindingTable, BranchFilter, BranchView};
pub use form::{
    all_definition_sites, form_at, form_in_module, module_is_registered, CanonicalSymbol,
    TypingForm,
};
pub use registry::registry_health;
pub use relation::{assignable, equivalent};
pub use type_node::{BuiltinClass, LiteralValue, TypeNode};
