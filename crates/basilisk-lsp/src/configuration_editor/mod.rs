//! LSP-owned configuration editor backend.
//!
//! Implements [CONFIGEDITOR-OPERATIONS] / [LSPARCH-CONFIG-EDITOR].

mod catalog;
pub mod model;
mod mutation;
mod protocol;
mod snapshot;
mod state;
mod transaction;

pub(crate) use state::ConfigurationEditorState;
pub(crate) use transaction::{
    apply_rule_updates, configuration_document, refresh_after_configuration_change,
};
