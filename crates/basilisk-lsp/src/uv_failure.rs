//! Implements [LSPUV-COMMAND-FAILURE-UX]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV-COMMAND-FAILURE-UX
//! uv failure classification — plain-language headlines instead of resolver dumps.
//!
//! When a uv subprocess fails, the raw resolver stderr is unactionable for a
//! typical Python developer (issue #94). This module maps common uv outcomes
//! to a short headline plus a remediation hint. The full stderr always stays
//! available in the Basilisk Output channel — only the toast is classified.

/// Broad category of a uv command failure, used for structured logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvFailureCategory {
    /// The requested package does not exist (or has no compatible version).
    PackageNotFound,
    /// The resolver found the package but it conflicts with existing pins.
    ResolutionConflict,
    /// The package index could not be reached.
    NetworkError,
    /// The `uv` binary itself is missing from `PATH`.
    UvNotFound,
    /// Anything else — point the user at the Output channel.
    Generic,
}

impl UvFailureCategory {
    /// Stable lowercase name for structured log fields.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PackageNotFound => "package_not_found",
            Self::ResolutionConflict => "resolution_conflict",
            Self::NetworkError => "network_error",
            Self::UvNotFound => "uv_not_found",
            Self::Generic => "generic",
        }
    }
}

/// A classified uv failure: what to show the user and how to log it.
#[derive(Debug, Clone)]
pub struct UvFailure {
    /// Failure category (also a structured log field).
    pub category: UvFailureCategory,
    /// One-sentence plain-language headline plus remediation for the toast.
    pub message: String,
}

/// Classify a failed uv command from its (ANSI-stripped) stderr.
///
/// `label` is the human-readable command (e.g. `"uv add six"`); `package` is
/// the package argument when the command has one.
//
// Implements [LSPUV-COMMAND-FAILURE-UX] — maps stderr to the spec's failure
// categories: `package_not_found` (`No solution found` + a not-found marker),
// `resolution_conflict` (`No solution found` alone), `network_error`, and
// `generic`. The toast is the plain-language headline + remediation; the full
// stderr stays in the Output channel (handled by the caller). NOTE: an extra
// not-found marker (`not found in the provided package locations`) beyond the
// two in the spec table is matched here — additive precision, no deviation.
#[must_use]
pub fn classify_uv_failure(label: &str, package: Option<&str>, stderr: &str) -> UvFailure {
    // uv hard-wraps its resolver prose, so a phrase like "was not found in the
    // package registry" can straddle a line break — collapse all whitespace
    // runs before matching.
    let stderr = stderr.split_whitespace().collect::<Vec<_>>().join(" ");
    let no_solution = stderr.contains("No solution found");
    let not_found = stderr.contains("was not found in the package registry")
        || stderr.contains("there are no versions of")
        || stderr.contains("not found in the provided package locations");

    if no_solution && not_found {
        let name = package.unwrap_or("the package");
        return UvFailure {
            category: UvFailureCategory::PackageNotFound,
            message: format!(
                "Package `{name}` couldn't be found or has no compatible version. \
                 Check the package name for a typo, or confirm it exists on the \
                 configured index. Full uv output is in the Basilisk Output channel."
            ),
        };
    }

    if no_solution {
        let name = package.unwrap_or("the package");
        return UvFailure {
            category: UvFailureCategory::ResolutionConflict,
            message: format!(
                "`{name}` conflicts with your existing dependencies. Relax a version \
                 pin, or retry with `--frozen` to skip locking. Full uv output is in \
                 the Basilisk Output channel."
            ),
        };
    }

    let lowered = stderr.to_lowercase();
    if lowered.contains("connection refused")
        || lowered.contains("error sending request")
        || lowered.contains("network")
        || lowered.contains("could not connect")
        || lowered.contains("dns error")
        || lowered.contains("operation timed out")
    {
        return UvFailure {
            category: UvFailureCategory::NetworkError,
            message: "Couldn't reach the package index. Check your network or index \
                      URL, then retry. Full uv output is in the Basilisk Output channel."
                .to_owned(),
        };
    }

    UvFailure {
        category: UvFailureCategory::Generic,
        message: format!("{label} failed. See the Basilisk Output channel for the full uv output."),
    }
}

/// Classification for a uv process that could not even be spawned.
//
// Implements [LSPUV-COMMAND-FAILURE-UX] — the `uv_not_found` category
// (`ErrorKind::NotFound` → "`uv` isn't installed or isn't on PATH" + install
// link), falling back to `generic` for any other spawn error.
#[must_use]
pub fn classify_uv_spawn_failure(err: &std::io::Error) -> UvFailure {
    if err.kind() == std::io::ErrorKind::NotFound {
        UvFailure {
            category: UvFailureCategory::UvNotFound,
            message: "`uv` isn't installed or isn't on PATH. Install it from \
                      https://docs.astral.sh/uv/getting-started/installation/ and retry."
                .to_owned(),
        }
    } else {
        UvFailure {
            category: UvFailureCategory::Generic,
            message: format!(
                "uv could not be started ({err}). See the Basilisk Output channel for details."
            ),
        }
    }
}
