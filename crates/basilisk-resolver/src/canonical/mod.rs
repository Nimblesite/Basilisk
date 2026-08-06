//! Implements [RESOLV-CANONICAL].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! Canonical symbol resolution — how Basilisk recognises specification forms.
//!
//! Recognition is a question about DEFINITIONS, not about characters. A use
//! site is resolved through the module's imports and bindings to the
//! definition it refers to ([`binding`]), and that definition site is looked up
//! in the specification registry ([`form`]).
//!
//! The Python spellings identifying each definition site live in
//! `resources/typing_symbols.toml`. No Rust file in this workspace contains
//! them, and `basilisk-checker/tests/no_symbol_naming.rs` fails the build if
//! one reappears.

mod binding;
mod form;

pub use binding::BindingTable;
pub use form::{
    all_definition_sites, form_at, form_in_module, module_is_registered, CanonicalSymbol,
    TypingForm,
};
