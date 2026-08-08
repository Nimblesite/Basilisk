//! Implements [CHKARCH-CONFIG-MODEL] and [CHKARCH-CONFIG-DISCOVERY]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL
//!
//! Configuration parsing for Basilisk.
//!
//! Parses `pyproject.toml` `[tool.basilisk]`. A configuration is two flat
//! maps and nothing else ([CHKARCH-CONFIG-MODEL]):
//! - `[tool.basilisk.rules]` — explicit per-rule severity entries
//! - `[tool.basilisk.rule-tags]` — explicit group entries
//!
//! Resolution is per rule, per checked file: the nearest table on the
//! ancestor walk that decides the rule wins outright
//! ([`BasiliskConfig::resolve_severity`]).
//!
//! Alongside the rule tables sit the flat project settings — the typeshed
//! source keys ([STUBRES-TYPESHED-CONFIG]), the version/platform target, and
//! the persistent result-cache keys `cache`/`cache-dir` ([CHKCACHE-CONFIG]).
//! Every key here is read by a real consumer; a setting with no reader does
//! not belong in this file (see `retired_auto_stub_keys_are_not_resurrected…`).

pub mod editor;
mod parse;
mod paths;
mod severity;

pub use editor::{
    active_config_path, apply_config_patch, build_configuration_patch, build_rule_patch,
    discover_config_document, discover_config_document_with_content, CacheConfigMutation,
    CacheConfigUpdate, ConfigDocument, ConfigDocumentError, ConfigPatch, ConfigurationUpdate,
    RuleConfigUpdate, TypeshedConfigKey, TypeshedConfigUpdate,
};
pub use parse::{
    is_full_commit_sha, is_valid_distribution_name, parse_typeshed_package, BasiliskConfig,
    RuleTables, DEFAULT_CACHE_DIR,
};
pub use paths::{is_virtualenv_dir, path_matches_pattern};
pub use severity::RuleSeverity;

use std::path::Path;

/// Directories excluded from analysis by default.
///
/// Users can override this via the `exclude` key in
/// `pyproject.toml [tool.basilisk]`. Setting `exclude` in config replaces
/// these defaults entirely — add them back explicitly if still needed.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "__pycache__",
    "node_modules",
    "venv",
    ".venv",
    "env",
    ".env",
    ".tox",
    ".mypy_cache",
    ".ruff_cache",
    ".pytest_cache",
    "site-packages",
    "__pypackages__",
    "build",
    "dist",
    ".eggs",
    // Vendored / bundled third-party code shipped verbatim (e.g. the extension's
    // `bundled/debugpy` tree and its nested `_vendored/`). Never our code to
    // type-check; scanning it floods thousands of irrelevant diagnostics (#80).
    "bundled",
    "_vendored",
];

/// Load a `BasiliskConfig` for `start` by discovering config files up the
/// ancestor directory chain.
///
/// Implements [CHKARCH-CONFIG-DISCOVERY] (GitHub #311): every surface —
/// `basilisk check`/`analyze`/`fix`/`adopt` and the LSP — resolves rule config
/// through this one routine, so the result is independent of argument order,
/// path spelling, and cwd. The walk visits `start` and every ancestor up to
/// the filesystem root; each directory contributes its `[tool.basilisk]`
/// table to the nearest-first [`BasiliskConfig::rule_chain`], and non-rule
/// fields merge with directories nearer to `start` winning per key. A
/// `pyproject.toml` without a `[tool.basilisk]` table contributes nothing and
/// does not stop the walk.
///
/// Returns `BasiliskConfig::default()` (empty chain — PEP rules at `error`,
/// nothing else runs; [CHKARCH-CONFIG-MODEL]) if no config table is found
/// anywhere on the chain. Treating a malformed file the same way is the open
/// CLI contract violation [#227](https://github.com/Nimblesite/Basilisk/issues/227):
/// malformed configuration must surface to command callers as exit code 2,
/// not silently become defaults.
#[must_use]
pub fn load_basilisk_config(start: &Path) -> BasiliskConfig {
    let chain: Vec<BasiliskConfig> = absolute_start(start)
        .ancestors()
        .filter_map(load_dir_config)
        .collect();
    // `ancestors()` yields nearest-first; fold from the outermost ancestor so
    // configs nearer to `start` end up in front of the rule chain.
    let mut config = chain
        .into_iter()
        .rev()
        .fold(BasiliskConfig::default(), BasiliskConfig::merged_with);
    // The nearest config-holding directory anchors root-relative
    // interpretation (`include`/`exclude` globs); with no config anywhere on
    // the chain, `start` itself anchors, as before.
    if config.project_root.is_none() {
        config.project_root = Some(start.to_path_buf());
    }
    config
}

/// Load the discovered-config chain for `start` bounded to directories
/// strictly below `stop` (exclusive): nested child configs only, never
/// `stop`'s own config file.
///
/// Implements [LSPARCH-CONFIG] via [CHKARCH-CONFIG-DISCOVERY]: a live server
/// holds the authoritative effective config for each workspace root in memory
/// — an applied configuration-editor change or an open, unsaved config buffer
/// is authoritative over whatever is currently on disk — so the root's file
/// must NOT be re-read here and merged back over it. Callers merge this
/// result over that in-memory root config. With no nested config on the
/// chain, the result is `BasiliskConfig::default()` with no `project_root`,
/// so the merge keeps the root config's own anchoring.
#[must_use]
pub fn load_basilisk_config_below(start: &Path, stop: &Path) -> BasiliskConfig {
    let chain: Vec<BasiliskConfig> = absolute_start(start)
        .ancestors()
        .take_while(|dir| *dir != stop)
        .filter_map(load_dir_config)
        .collect();
    // `ancestors()` yields nearest-first; fold from the outermost ancestor so
    // configs nearer to `start` end up in front of the rule chain.
    chain
        .into_iter()
        .rev()
        .fold(BasiliskConfig::default(), BasiliskConfig::merged_with)
}

/// The nearest directory at or above `start` holding a recognized config file
/// (`pyproject.toml` with a `[tool.basilisk]` table).
///
/// This anchors artifacts that live next to the config, so `basilisk adopt`
/// writes where `basilisk check` discovers. Implements
/// [CHKARCH-CONFIG-DISCOVERY].
#[must_use]
pub fn discover_config_dir(start: &Path) -> Option<std::path::PathBuf> {
    absolute_start(start)
        .ancestors()
        .find(|dir| load_dir_config(dir).is_some())
        .map(Path::to_path_buf)
}

/// Load the config from exactly one directory — no ancestor walk.
///
/// Implements [CHKARCH-CONFIG-FILE]: the only configuration source is the
/// `[tool.basilisk]` table of the directory's `pyproject.toml`. A
/// `pyproject.toml` without that table contributes nothing.
///
/// Returns `None` when the directory holds no parseable config.
fn load_dir_config(dir: &Path) -> Option<BasiliskConfig> {
    let pyproject = dir.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(mut cfg) = parse::load_from_pyproject(&pyproject) {
            cfg.project_root = Some(dir.to_path_buf());
            return Some(cfg);
        }
    }

    None
}

/// Absolutize `start` (against cwd) so `ancestors()` walks the full directory
/// chain even for relative paths like `.` or `child/` — WITHOUT
/// canonicalizing, so discovered directories keep the caller's path spelling
/// (a symlinked temp dir must not come back re-rooted, or callers that
/// `strip_prefix` against their own paths break).
fn absolute_start(start: &Path) -> std::path::PathBuf {
    if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| start.to_path_buf(), |cwd| cwd.join(start))
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
#[path = "lib_tests.rs"]
mod tests;
