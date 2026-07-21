//! Implements the [STUBRES-TYPESHED-DOWNLOAD] CLI surface:
//! `basilisk typeshed download [--commit <sha>]`.
//!
//! This command — like the editor's Download buttons — is the ONLY way
//! typeshed bytes arrive on a machine ([TYPESHEDRT-SEGREGATION]). `check` and
//! `analyze` never download: a pin that is not in the store tanks hard with
//! `NO SOURCE`, and this command is what that error tells the user to run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use basilisk_typeshed_fetch::{DownloadPhase, GithubClient};
use colored::Colorize as _;
use tracing::error;

/// The `basilisk typeshed` action surface.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum TypeshedAction {
    /// Download and verify one typeshed commit into the content-addressed
    /// store. With no `--commit` this resolves the latest
    /// `python/typeshed@main` and writes the resolved SHA as the workspace's
    /// `typeshed-commit` pin; with `--commit` it materialises that exact,
    /// already-configured pin and writes no configuration.
    Download {
        /// Exact full 40-hex commit SHA to download (defaults to latest).
        #[arg(long, value_name = "SHA")]
        commit: Option<String>,
        /// Workspace whose configuration supplies the store location and, for
        /// a latest download, receives the pin.
        #[arg(long, default_value = ".", value_name = "DIR")]
        workspace: PathBuf,
    },
}

/// Dispatch a `basilisk typeshed` action. Returns the process exit code
/// ([CHKARCH-CLI-EXITCODES]: `0` ok, `2` invalid configuration, `3` failure).
pub(crate) fn run(action: TypeshedAction) -> u8 {
    match action {
        TypeshedAction::Download { commit, workspace } => run_download(commit, &workspace),
    }
}

fn run_download(commit: Option<String>, workspace: &Path) -> u8 {
    let config = basilisk_lsp::config::load_analysis_config(workspace);
    let store = config.typeshed_store_path;
    let client = GithubClient::new();
    let progress = |phase: DownloadPhase| println!("  {}", phase_label(phase).dimmed());
    match commit {
        Some(sha) => download_exact(&sha, store, &client, &progress),
        None => download_latest_and_pin(workspace, store, &client, &progress),
    }
}

fn download_exact(
    sha: &str,
    store: Option<PathBuf>,
    client: &GithubClient,
    progress: &dyn Fn(DownloadPhase),
) -> u8 {
    let Ok(commit) = basilisk_stubs::typeshed::gittree::Oid::from_hex(sha) else {
        error!(
            len = sha.len(),
            "--commit must be a full 40-character hex SHA"
        );
        return 2;
    };
    println!("Downloading typeshed {commit} into the verified store…");
    match basilisk_typeshed_fetch::download_commit(commit, store, client, progress) {
        Ok(outcome) => {
            println!(
                "{} {} (tree {}) is now available offline",
                "ok:".green().bold(),
                outcome.commit,
                outcome.tree
            );
            0
        }
        Err(download_error) => {
            error!(%download_error, "typeshed download failed; nothing was written");
            3
        }
    }
}

fn download_latest_and_pin(
    workspace: &Path,
    store: Option<PathBuf>,
    client: &GithubClient,
    progress: &dyn Fn(DownloadPhase),
) -> u8 {
    println!("Downloading the latest python/typeshed commit into the verified store…");
    let outcome = match basilisk_typeshed_fetch::download_latest(store, client, progress) {
        Ok(outcome) => outcome,
        Err(download_error) => {
            error!(%download_error, "typeshed download failed; nothing was written");
            return 3;
        }
    };
    match write_pin(workspace, &outcome.commit.to_hex()) {
        Ok(()) => {
            println!(
                "{} pinned typeshed-commit = {}",
                "ok:".green().bold(),
                outcome.commit
            );
            0
        }
        Err(config_error) => {
            // The store entry is verified and kept — only the pin write
            // failed, so the command is re-runnable without a re-download.
            error!(%config_error, commit = %outcome.commit, "downloaded but could not write the pin");
            2
        }
    }
}

/// Write `typeshed-commit` through the same validated, structure-preserving
/// editor transaction the LSP configuration editor uses ([LSPCFGED-TYPESHED]).
fn write_pin(workspace: &Path, sha: &str) -> Result<(), basilisk_config::ConfigDocumentError> {
    let document = basilisk_config::discover_config_document(workspace)?;
    let update = basilisk_config::ConfigurationUpdate {
        rules: basilisk_config::RuleConfigUpdate::default(),
        typeshed: basilisk_config::TypeshedConfigUpdate {
            entries: BTreeMap::from([(
                basilisk_config::TypeshedConfigKey::TypeshedCommit,
                Some(sha.to_owned()),
            )]),
        },
    };
    let patch = basilisk_config::build_configuration_patch(&document, &update)?;
    basilisk_config::apply_config_patch(&patch)
}

const fn phase_label(phase: DownloadPhase) -> &'static str {
    match phase {
        DownloadPhase::Resolving => "resolving commit metadata",
        DownloadPhase::FetchingTree => "fetching the trusted file tree",
        DownloadPhase::FetchingArchive => "downloading the archive",
        DownloadPhase::Verifying => "verifying against the commit identity",
        DownloadPhase::Writing => "writing the store entry",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [STUBRES-TYPESHED-DOWNLOAD]: the pin write is the same validated
    /// editor transaction the configuration editor uses — structure
    /// preserved, full SHA required.
    #[test]
    fn write_pin_round_trips_through_the_validated_editor() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "# keep\n[project]\nname = \"demo\"\n\n[tool.basilisk]\n",
        )?;
        write_pin(dir.path(), "83c2518a9e6abbda0c44592c3483de459198f887")?;
        let written = std::fs::read_to_string(dir.path().join("pyproject.toml"))?;
        assert!(written.contains("# keep"));
        assert!(
            written.contains("typeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\""),
            "pin must be written: {written}"
        );

        assert!(
            write_pin(dir.path(), "not-a-sha").is_err(),
            "a malformed SHA must be rejected by validation, never written"
        );
        Ok(())
    }

    #[test]
    fn a_malformed_commit_argument_is_a_configuration_error() {
        let client = GithubClient::new();
        // No network is touched: the SHA fails validation before any request.
        assert_eq!(download_exact("short", None, &client, &|_phase| {}), 2);
    }
}
