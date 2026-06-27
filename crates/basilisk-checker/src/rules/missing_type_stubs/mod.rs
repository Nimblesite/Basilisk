//! Implements [BSK-E0152] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0152: Missing type stubs for installed package.
//!
//! Fires when a package is imported and resolves to a `.py` source file (not
//! `.pyi`) without a `py.typed` marker. This means the package is installed
//! but lacks type information, reducing type safety. This rule is off by
//! default — the default configuration is pure PEP conformance — and a project
//! opts in via configuration (`uv.stubSuggestions`). Once enabled, an untyped
//! third-party import is a hard error; a project can soften it per import
//! (`# type: warning[BSK-E0152]`) or globally (`"BSK-E0152" = "warning"`) to
//! use non-type-safe libraries at its own risk.
//!
//! ```python
//! import flask  # E0152: Package 'flask' is installed but has no type stubs
//! ```

use basilisk_resolver::{ImportInfo, ImportResolution, ResolvedModule};
use basilisk_stubs::TypeProvenance;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
mod tests;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0152",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0152",
};

/// Emits BSK-E0152 when an imported package resolves to a `.py` source file
/// without a `py.typed` marker, indicating missing type stubs.
pub(crate) struct MissingTypeStubs;

impl Rule for MissingTypeStubs {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["stubs"],
        })
    }

    // Implements [STUBRES-PEP561] step 6 (no stubs found) — fires the BSK-E0152
    // import-site diagnostic only for a site-packages `.py` import that is not
    // stdlib and carries no PEP 561 `py.typed` marker (i.e. resolution exhausted
    // steps 1-5 without type information).
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .imports
            .iter()
            .filter(|import| import.resolution == ImportResolution::SourcePy)
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| is_site_packages_import(import))
            .filter(|import| !has_py_typed_marker(import))
            .for_each(|import| diagnostics.push(make_diagnostic(import, &module.path)));
    }
}

/// Check whether an import resolved to a site-packages path.
///
/// A simple heuristic: the resolved path contains a `site-packages` component.
fn is_site_packages_import(import: &ImportInfo) -> bool {
    import
        .resolved_path
        .as_ref()
        .is_some_and(|path| path.to_string_lossy().contains("site-packages"))
}

/// Check whether the resolved package has a `py.typed` marker (PEP 561).
///
/// Per PEP 561 the marker is placed at the **top-level package** and applies
/// to every submodule. A submodule may resolve either to a flat-file module
/// (`.../pydantic_ai/direct.py`) or to a nested subpackage
/// (`.../sqlalchemy/orm/__init__.py`); in both cases the marker lives at the
/// top-level package root (`.../pydantic_ai/py.typed`, `.../sqlalchemy/py.typed`).
///
/// We walk the filesystem upward from the resolved file toward the
/// `site-packages` root, returning `true` at the first directory that contains
/// a `py.typed` marker. Walking the directory tree (rather than counting dots
/// in the dotted module name) honors flat-file and subpackage submodules
/// uniformly and never over-climbs past the package into `site-packages`.
fn has_py_typed_marker(import: &ImportInfo) -> bool {
    import
        .resolved_path
        .as_ref()
        .is_some_and(|resolved| basilisk_stubs::has_py_typed_marker(resolved))
}

/// Build the diagnostic for a missing type stubs error.
// Implements [STUBRES-PROVENANCE-DIAG] — the `Untyped` provenance row: a single
// import-site diagnostic carrying `TypeProvenance::Untyped` so the checker can
// suppress the downstream cascade (cascade logic lives elsewhere; see report).
fn make_diagnostic(import: &ImportInfo, path: &str) -> Diagnostic {
    let root_module = import.module.split('.').next().unwrap_or(&import.module);

    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!("Package `{root_module}` is installed but has no type stubs available"),
        span: import.span,
        path: path.to_owned(),
        help: Some(stub_help_text(root_module)),
        note: Some(
            "Packages without type stubs or a PEP 561 `py.typed` marker provide no type \
             information — https://peps.python.org/pep-0561/"
                .to_owned(),
        ),
        provenance: Some(TypeProvenance::Untyped),
    }
}

/// Build the `help` line for a missing-stubs diagnostic.
///
/// When typeshed publishes a stub distribution for the package, point the user
/// at the real distribution name (e.g. `yaml` → `types-PyYAML`) so the quick fix
/// resolves. Otherwise spell out the local-stub escape hatch: a `.pyi` placed on
/// a `stub-paths` directory resolves *before* site-packages, so a hand-written
/// (or quick-fix-generated) stub silences this error. The link points at the
/// official authoring guide so a developer — or an AI assisting in the editor —
/// has self-contained context to write the stub. No shell command appears here:
/// per [STUBRES-CODEACTIONS] the quick fix does the work, the help only explains.
fn stub_help_text(root_module: &str) -> String {
    match basilisk_stubs::typeshed_stub_distribution(root_module) {
        Some(distribution) => {
            format!("Type stubs available as `{distribution}` — use quick fix to install")
        }
        None => format!(
            "No published type stubs for `{root_module}` — create a local stub \
             (`{root_module}.pyi` in a `stub-paths` directory) or upstream a PEP 561 `py.typed` \
             marker. Guide: https://typing.python.org/en/latest/guides/writing_stubs.html"
        ),
    }
}
