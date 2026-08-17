//! Implements [WITHDRAWAL-INERT]. See
//! docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT
//!
//! Basilisk's type checker is inert. It parses no arguments, reads no file,
//! starts no server, and checks nothing. Every invocation prints the approved
//! notice to stderr and exits `4`, so a pipeline that still calls Basilisk
//! breaks loudly instead of reading a clean run into a checker that was
//! producing incorrect results. `--version` is the sole exception: package
//! managers and installed editor extensions probe it, and a hang would hide
//! the notice rather than deliver it.

use std::io::Write as _;
use std::process::ExitCode;

use shipwright::{dispatch, BuildInfo, VersionSpec};
use shipwright_manifest::{ExecutableKind, Language};

/// The approved notice, verbatim. Generated from the messaging spec by
/// `scripts/gen_withdrawal_copy.py` ([WITHDRAWAL-INERT-TEXT]) and drift-gated
/// in CI, so this binary can never print its own version of the statement.
const NOTICE: &str = include_str!("withdrawal_notice.txt");

/// `4` — unlisted ([CHKARCH-CLI-EXITCODES]). Distinct from `1` ("error
/// diagnostics found", which would be one more incorrect result) and from `2`
/// and `3`, so a consumer can tell "Basilisk is gone" from "Basilisk failed".
const EXIT_UNLISTED: u8 = 4;

/// Answer `--version` / `--version --json` through the Shipwright contract.
///
/// Returns `true` when a version flag was handled and `main` should exit 0. The
/// capability list is empty and the kind is `Cli`: this binary is no longer a
/// language server, an MCP server, a debug adapter, or a profiler, and saying
/// otherwise to a tool that reads the contract would be a false claim.
fn handle_version(args: &[String]) -> bool {
    let spec = VersionSpec {
        name: "basilisk",
        version: env!("CARGO_PKG_VERSION"),
        kind: ExecutableKind::Cli,
        language: Language::Rust,
        product: Some("basilisk"),
        capabilities: &[],
        build: BuildInfo {
            git_sha: option_env!("SHIPWRIGHT_GIT_SHA"),
            git_dirty: option_env!("SHIPWRIGHT_GIT_DIRTY").map(|s| s == "true"),
            build_time: option_env!("SHIPWRIGHT_BUILD_TIME"),
            target: option_env!("SHIPWRIGHT_TARGET"),
            toolchain: option_env!("SHIPWRIGHT_TOOLCHAIN"),
        },
    };
    // A failed emission falls through to the notice below rather than being
    // reported: there is no successful outcome left for this binary to have.
    dispatch(args, &mut std::io::stdout(), &spec).unwrap_or(false)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if handle_version(&args) {
        return ExitCode::SUCCESS;
    }
    // The one deliberate direct write to stderr in the codebase (CLAUDE.md):
    // `tracing` would prefix, filter, and colourise the statement, and
    // BASILISK_LOG could suppress it entirely. Stdout stays empty so
    // `--output json > report.json` yields an empty file rather than prose a
    // consumer might parse as findings.
    let _ = std::io::stderr().write_all(NOTICE.as_bytes());
    ExitCode::from(EXIT_UNLISTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The notice is the spec's text, not a paraphrase, and it tells the reader
    /// the three things they must act on: it is unlisted, it checks nothing,
    /// and the failure is not about their code.
    #[test]
    fn notice_carries_the_approved_statement() {
        assert!(NOTICE.starts_with("Basilisk is unlisted."));
        assert!(NOTICE.contains("checks nothing"));
        assert!(NOTICE.contains("This command failed on purpose."));
        assert!(NOTICE.contains("https://github.com/python/typing/pull/2330"));
        assert!(NOTICE.ends_with("basilisk-conformance-apology\n"));
    }

    /// Only a version flag is answered. Every other argument shape — including
    /// the ones clap used to own, like `--help` — falls through to the notice.
    #[test]
    fn only_version_flags_are_handled() {
        assert!(handle_version(&["--version".to_owned()]));
        for args in [
            vec![],
            vec!["check".to_owned()],
            vec!["--help".to_owned()],
            vec!["--not-a-flag".to_owned()],
        ] {
            assert!(!handle_version(&args), "must not be handled: {args:?}");
        }
    }

    /// Exit `4` is neither success nor "errors found": a build that still calls
    /// Basilisk must fail, and must not read the failure as a code finding.
    /// The four codes it must not collide with are the checker's own
    /// ([CHKARCH-CLI-EXITCODES]).
    #[test]
    fn unlisted_exit_code_is_four_and_not_a_diagnostic_code() {
        assert_eq!(EXIT_UNLISTED, 4);
        let checker_codes = [0_u8, 1, 2, 3];
        assert!(!checker_codes.contains(&EXIT_UNLISTED));
    }
}
