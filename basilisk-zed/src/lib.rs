//! Basilisk extension for the Zed editor.
//!
//! Basilisk is unlisted and its type checker is inert, so this extension has
//! one job: state that, in the words the messaging spec approved. It launches
//! no language server, downloads no binary, and registers no debug adapter
//! ([WITHDRAWAL-SURFACES]).
//!
//! Pure logic lives in [`logic`] (testable on native target).
//! This file is thin glue that bridges [`logic`] ↔ `zed_extension_api` types.
#![expect(
    missing_docs,
    reason = "zed::register_extension! generates undocumented items"
)]

mod logic;

use zed_extension_api::{self as zed, Result};

struct BasiliskExtension;

// The `zed::Extension` impl + `register_extension!` below is the extension
// entry point. Implements [ZED-LIBRS]. Every other trait method keeps its
// default — the defaults return "not implemented", which is the honest answer
// for a server, adapter, or command this extension no longer provides.
impl zed::Extension for BasiliskExtension {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    /// `/basilisk` — print the approved statement into the assistant panel.
    fn run_slash_command(
        &self,
        _command: zed::SlashCommand,
        _args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<zed::SlashCommandOutput> {
        let (label, text) = logic::notice_output();
        Ok(zed::SlashCommandOutput {
            sections: vec![zed::SlashCommandOutputSection {
                range: (0..text.len()).into(),
                label,
            }],
            text,
        })
    }
}

zed::register_extension!(BasiliskExtension);
