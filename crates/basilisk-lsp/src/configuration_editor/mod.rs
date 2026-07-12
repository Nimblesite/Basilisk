//! LSP-owned configuration editor backend.
//!
//! Implements [CONFIGEDITOR-OPERATIONS] / [LSPARCH-CONFIG-EDITOR].

mod catalog;
pub mod model;
mod protocol;
mod snapshot;

pub(crate) use protocol::{ConfigurationEditorState, PreparedPreview};
