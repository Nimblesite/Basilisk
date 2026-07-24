//! Implements the [STUBRES-TYPESHED-DOWNLOAD] CLI surface:
//! `basilisk typeshed download [--commit <sha>]`.
//!
//! This command — like the editor's Download buttons — is the ONLY way
//! typeshed bytes arrive on a machine ([TYPESHEDRT-SEGREGATION]). `check` and
//! `analyze` never download: a pin that is not in the store tanks hard with
//! `NO SOURCE`, and this command is what that error tells the user to run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use basilisk_typeshed_fetch::{DownloadPhase, GithubApi, GithubClient};
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
    let client = GithubClient::new();
    download_action(commit, workspace, &client)
}

/// The download action with its transport injected, so tests drive the whole
/// surface — config discovery, store resolution, progress — offline.
fn download_action(commit: Option<String>, workspace: &Path, api: &dyn GithubApi) -> u8 {
    let config = basilisk_lsp::config::load_analysis_config(workspace);
    let store = config.typeshed_store_path;
    let progress = |phase: DownloadPhase| println!("  {}", phase_label(phase).dimmed());
    match commit {
        Some(sha) => download_exact(&sha, store, api, &progress),
        None => download_latest_and_pin(workspace, store, api, &progress),
    }
}

fn download_exact(
    sha: &str,
    store: Option<PathBuf>,
    api: &dyn GithubApi,
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
    match basilisk_typeshed_fetch::download_commit(commit, store, api, progress) {
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
    api: &dyn GithubApi,
    progress: &dyn Fn(DownloadPhase),
) -> u8 {
    println!("Downloading the latest python/typeshed commit into the verified store…");
    let outcome = match basilisk_typeshed_fetch::download_latest(store, api, progress) {
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
///
/// The pin and a custom folder are the two mutually exclusive step-3 sources
/// ([STUBRES-TYPESHED]), so the same transaction retires `typeshed-path` —
/// byte for byte the update the LSP's Download latest button writes
/// (`pin_update` in `crates/basilisk-lsp/src/typeshed_download.rs`). Without
/// the retirement the patch would name both sources and validation would
/// reject the whole write, leaving a downloaded commit unpinned.
fn write_pin(workspace: &Path, sha: &str) -> Result<(), basilisk_config::ConfigDocumentError> {
    let document = basilisk_config::discover_config_document(workspace)?;
    let update = basilisk_config::ConfigurationUpdate {
        rules: basilisk_config::RuleConfigUpdate::default(),
        typeshed: basilisk_config::TypeshedConfigUpdate {
            entries: BTreeMap::from([
                (
                    basilisk_config::TypeshedConfigKey::TypeshedCommit,
                    Some(sha.to_owned()),
                ),
                (basilisk_config::TypeshedConfigKey::TypeshedPath, None),
            ]),
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
    use basilisk_typeshed_fetch::testing::{fake_repo, FakeApi, Faults};

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

    /// [STUBRES-TYPESHED-DOWNLOAD]: pinning retires a custom folder in the
    /// same transaction, exactly like the LSP's Download latest action. The
    /// two step-3 sources are mutually exclusive, so a write that kept both
    /// would be rejected outright and the download would end up unpinned.
    #[test]
    fn write_pin_retires_a_custom_typeshed_path() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.basilisk]\ntypeshed-path = \"vendor/typeshed\"\ntypeshed-store-path = \"store\"\n",
        )?;
        write_pin(dir.path(), "83c2518a9e6abbda0c44592c3483de459198f887")?;
        let written = std::fs::read_to_string(dir.path().join("pyproject.toml"))?;
        assert!(
            written.contains("typeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\""),
            "the resolved pin must be written: {written}"
        );
        assert!(
            !written.contains("typeshed-path"),
            "the custom folder must be retired by the same write: {written}"
        );
        assert!(
            written.contains("typeshed-store-path = \"store\""),
            "unrelated typeshed settings must survive untouched: {written}"
        );
        Ok(())
    }

    #[test]
    fn a_malformed_commit_argument_is_a_configuration_error() {
        let api = FakeApi::new(fake_repo());
        // No transport is touched: the SHA fails validation before any request.
        assert_eq!(download_exact("short", None, &api, &|_phase| {}), 2);
    }

    /// The `run` dispatch reaches the same validation: a malformed pin exits
    /// `2` before any transport work.
    #[test]
    fn run_rejects_a_malformed_sha_through_the_dispatch() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let action = TypeshedAction::Download {
            commit: Some("short".to_owned()),
            workspace: dir.path().to_path_buf(),
        };
        assert_eq!(run(action), 2);
        Ok(())
    }

    /// `download --commit <sha>` materialises the exact pin into the store and
    /// writes no configuration ([STUBRES-TYPESHED-DOWNLOAD]).
    #[test]
    fn download_exact_materialises_the_pin_into_the_store() -> Result<(), Box<dyn std::error::Error>>
    {
        let store = tempfile::tempdir()?;
        let api = FakeApi::new(fake_repo());
        let sha = api.repo.commit.to_hex();
        assert_eq!(
            download_exact(&sha, Some(store.path().to_path_buf()), &api, &|_phase| {}),
            0
        );
        assert_eq!(
            std::fs::read_dir(store.path())?.count(),
            1,
            "exactly one verified store entry must exist"
        );
        Ok(())
    }

    /// A transport failure is exit `3` and writes nothing.
    #[test]
    fn a_transport_failure_downloading_an_exact_pin_is_exit_3(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let store = tempfile::tempdir()?;
        let mut api = FakeApi::new(fake_repo());
        api.faults = Faults {
            resolve_fails: true,
            ..Faults::default()
        };
        let sha = api.repo.commit.to_hex();
        assert_eq!(
            download_exact(&sha, Some(store.path().to_path_buf()), &api, &|_phase| {}),
            3
        );
        assert_eq!(std::fs::read_dir(store.path())?.count(), 0);
        Ok(())
    }

    /// `download` with no `--commit` resolves latest, stores it, and pegs the
    /// resolved SHA as the workspace's `typeshed-commit` pin.
    #[test]
    fn download_latest_pins_the_resolved_sha() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let store = tempfile::tempdir()?;
        std::fs::write(workspace.path().join("pyproject.toml"), "[tool.basilisk]\n")?;
        let api = FakeApi::new(fake_repo());
        assert_eq!(
            download_latest_and_pin(
                workspace.path(),
                Some(store.path().to_path_buf()),
                &api,
                &|_phase| {}
            ),
            0
        );
        let written = std::fs::read_to_string(workspace.path().join("pyproject.toml"))?;
        assert!(
            written.contains(&format!("typeshed-commit = \"{}\"", api.repo.commit)),
            "the resolved SHA must be pegged as the pin: {written}"
        );
        Ok(())
    }

    /// A failed latest download is exit `3` and leaves the configuration
    /// untouched — no pin without verified bytes.
    #[test]
    fn a_failed_latest_download_writes_no_pin() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let store = tempfile::tempdir()?;
        std::fs::write(workspace.path().join("pyproject.toml"), "[tool.basilisk]\n")?;
        let mut api = FakeApi::new(fake_repo());
        api.faults = Faults {
            archive_fails: true,
            ..Faults::default()
        };
        assert_eq!(
            download_latest_and_pin(
                workspace.path(),
                Some(store.path().to_path_buf()),
                &api,
                &|_phase| {}
            ),
            3
        );
        let written = std::fs::read_to_string(workspace.path().join("pyproject.toml"))?;
        assert!(
            !written.contains("typeshed-commit"),
            "no pin may be written for a failed download: {written}"
        );
        Ok(())
    }

    /// A pin-write failure after a successful download is exit `2`; the store
    /// entry is kept so re-running needs no re-download.
    #[cfg(unix)]
    #[test]
    fn a_pin_write_failure_after_download_is_a_configuration_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;
        let workspace = tempfile::tempdir()?;
        let store = tempfile::tempdir()?;
        std::fs::write(workspace.path().join("pyproject.toml"), "[tool.basilisk]\n")?;
        let api = FakeApi::new(fake_repo());
        std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o555))?;
        let exit = download_latest_and_pin(
            workspace.path(),
            Some(store.path().to_path_buf()),
            &api,
            &|_phase| {},
        );
        std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o755))?;
        assert_eq!(exit, 2);
        assert_eq!(
            std::fs::read_dir(store.path())?.count(),
            1,
            "the verified store entry must survive the failed pin write"
        );
        Ok(())
    }

    /// The full action surface offline: config discovery resolves the
    /// workspace-relative store, the latest commit lands there, and the pin is
    /// pegged — the exact flow `basilisk typeshed download` runs.
    #[test]
    fn download_action_resolves_the_store_from_workspace_config(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        std::fs::write(
            workspace.path().join("pyproject.toml"),
            "[tool.basilisk]\ntypeshed-store-path = \"store\"\n",
        )?;
        let api = FakeApi::new(fake_repo());
        assert_eq!(download_action(None, workspace.path(), &api), 0);
        assert_eq!(
            std::fs::read_dir(workspace.path().join("store"))?.count(),
            1,
            "the store entry must land in the config-resolved location"
        );
        Ok(())
    }

    /// Every phase renders a distinct, human-readable progress label.
    #[test]
    fn every_download_phase_has_a_distinct_label() {
        let labels = [
            phase_label(DownloadPhase::Resolving),
            phase_label(DownloadPhase::FetchingTree),
            phase_label(DownloadPhase::FetchingArchive),
            phase_label(DownloadPhase::Verifying),
            phase_label(DownloadPhase::Writing),
        ];
        let unique: std::collections::BTreeSet<&str> = labels.iter().copied().collect();
        assert_eq!(unique.len(), labels.len());
        assert!(labels.iter().all(|label| !label.is_empty()));
    }
}
