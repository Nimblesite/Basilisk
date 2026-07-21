//! LSP-owned configuration editor backend.
//!
//! Implements [CONFIGEDITOR-OPERATIONS] / [LSPARCH-CONFIG-EDITOR].

mod catalog;
pub mod model;
mod mutation;
mod protocol;
mod snapshot;
pub(crate) mod snapshot_typeshed;
mod state;
mod transaction;
mod typeshed_resolution;
mod watch;

pub(crate) use state::ConfigurationEditorState;
pub(crate) use transaction::{
    apply_configuration_update, apply_rule_updates, configuration_document,
    refresh_after_configuration_change, ConfigurationRefreshHandles,
};
pub(crate) use typeshed_resolution::{resolve_and_activate, resolve_workspace};
pub(crate) use watch::{
    refresh_environment_from_disk, refresh_root_from_disk, spawn_configuration_watcher,
};
