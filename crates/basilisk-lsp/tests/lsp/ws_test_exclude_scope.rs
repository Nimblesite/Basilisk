//! Tests for [CHKARCH-CONFIG-EXCLUDE].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE
//!
//! End-to-end proof, over the real WebSocket server, that the editor's startup
//! scan resolves `exclude` through the SAME model as `basilisk check`
//! (`crates/basilisk-cli/tests/e2e_exclude_config.rs`):
//!
//! - unset `exclude` → [`basilisk_config::DEFAULT_EXCLUDES`] apply;
//! - set `exclude` → it REPLACES those defaults rather than extending them.
//!
//! The second rule is what a hardcoded default set inside
//! `workspace_scan::collect_python_files` used to break: no configuration
//! could switch it off, so the editor silently skipped files `basilisk check`
//! reported on.

use super::ws_test_common::*;

use std::time::Duration;

use futures_util::StreamExt;

/// Missing parameter/return annotations — fires `BSK-0001`/`BSK-0002`, which
/// the fixture config opts into ([CHKARCH-CONFIGURATION-ONLY]).
const UNANNOTATED: &str = "def greet(name):\n    return name\n";

/// House rules on, so any *scanned* file has something to publish.
const RULES: &str = "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n";

fn seed(dir: &std::path::Path, rel: &str) -> TestResult<()> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, UNANNOTATED)?;
    Ok(())
}

/// How long to wait for the startup scan's FIRST publish. The server boots,
/// resolves its stub snapshot and indexes the workspace before anything reaches
/// the wire, and on a cold CI runner that is orders of magnitude slower than on
/// a warm laptop.
const FIRST_PUBLISH_TIMEOUT: Duration = Duration::from_mins(1);

/// How long the stream must stay silent AFTER the sentinel before the scan
/// counts as finished. Generous, because it is paid once per test and a scan
/// that publishes its files a beat apart must not be cut in half.
const QUIET_AFTER_SENTINEL: Duration = Duration::from_secs(3);

/// Drain the startup scan and report which of `names` got a NON-EMPTY
/// `publishDiagnostics`.
///
/// `sentinel` names the one file the configuration under test must always
/// scan. Waiting for it is what makes an ABSENCE meaningful: "excluded" and
/// "not published yet" are the same thing on the wire — silence — so a fixed
/// quiet period cannot tell them apart. It only encodes a guess about how fast
/// the machine is, and that guess is wrong on the run that matters. So wait as
/// long as it takes for the scan to prove it is alive, and only then treat
/// silence as the answer.
async fn scanned_files(fixture: &mut WsTestFixture, names: &[&str], sentinel: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let start = std::time::Instant::now();
    loop {
        let patience = if seen.iter().any(|found| found == sentinel) {
            QUIET_AFTER_SENTINEL
        } else {
            FIRST_PUBLISH_TIMEOUT.saturating_sub(start.elapsed())
        };
        if patience.is_zero() {
            break;
        }
        let next = tokio::time::timeout(patience, fixture.ws_read.next()).await;
        let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) = next else {
            break;
        };
        if !text.contains("\"method\":\"textDocument/publishDiagnostics\"")
            || !text.contains("\"diagnostics\":[{")
        {
            continue;
        }
        for name in names {
            if text.contains(name) && !seen.iter().any(|found| found == name) {
                seen.push((*name).to_owned());
            }
        }
    }
    seen
}

// Configuring `exclude` REPLACES the defaults, so a default-excluded tree is
// scanned again — exactly as `basilisk check` scans it. Before the fix the
// scanner applied DEFAULT_EXCLUDES unconditionally and `build/` stayed dark.
#[tokio::test]
async fn test_ws_configured_exclude_replaces_defaults_like_the_cli() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_ws_exclude_replaces");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("pyproject.toml"),
        format!("[tool.basilisk]\nexclude = [\"vendor\"]\n\n{RULES}"),
    )?;
    seed(&dir, "build/generated.py")?;
    seed(&dir, "vendor/thirdparty.py")?;
    seed(&dir, "app.py")?;

    let root_uri = format!("file://{}", dir.display());
    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    let seen = scanned_files(
        &mut fixture,
        &["build/generated.py", "vendor/thirdparty.py", "app.py"],
        "app.py",
    )
    .await;

    assert!(
        seen.iter().any(|name| name == "app.py"),
        "the startup scan must publish for an ordinary source; saw {seen:?}"
    );
    assert!(
        seen.iter().any(|name| name == "build/generated.py"),
        "`exclude = [\"vendor\"]` drops DEFAULT_EXCLUDES, so `build/` must be \
         scanned exactly as `basilisk check` scans it; saw {seen:?}"
    );
    assert!(
        !seen.iter().any(|name| name == "vendor/thirdparty.py"),
        "the configured `vendor` pattern must still be honoured; saw {seen:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// The complement: with `exclude` unset the defaults still apply, so removing
// the hardcoded set did not remove the protection it used to provide.
#[tokio::test]
async fn test_ws_unset_exclude_still_skips_default_excluded_trees() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_ws_exclude_defaults");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("pyproject.toml"), RULES)?;
    seed(&dir, "build/generated.py")?;
    seed(&dir, "node_modules/dep.py")?;
    seed(&dir, "app.py")?;

    let root_uri = format!("file://{}", dir.display());
    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    let seen = scanned_files(
        &mut fixture,
        &["build/generated.py", "node_modules/dep.py", "app.py"],
        "app.py",
    )
    .await;

    assert_eq!(
        seen,
        vec!["app.py".to_owned()],
        "with `exclude` unset, DEFAULT_EXCLUDES must keep vendored trees out of \
         the scan; saw {seen:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
