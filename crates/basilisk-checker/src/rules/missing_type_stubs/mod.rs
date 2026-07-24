//! Implements [BSK-0152] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! BSK-0152: Missing type stubs for installed package.
//!
//! Fires when a package is imported and resolves to a `.py` source file (not
//! `.pyi`) without a `py.typed` marker. This means the package is installed
//! but lacks type information, reducing type safety. This rule is off by
//! default — the default configuration is pure PEP conformance — and a project
//! opts in with an explicit `BSK-0152` severity. Once enabled, an untyped
//! third-party import is a hard error; a project can soften it per import
//! (`# type: warning[BSK-0152]`) or globally (`"BSK-0152" = "warning"`) to
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
    code: "BSK-0152",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0152",
};

/// Emits BSK-0152 when an imported package resolves to a `.py` source file
/// without a `py.typed` marker, indicating missing type stubs.
pub(crate) struct MissingTypeStubs;

impl Rule for MissingTypeStubs {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["stubs"],
        })
    }

    // Implements [STUBRES-PEP561] after step 6 is exhausted — fires BSK-0152
    // import-site diagnostic only for a site-packages `.py` import that is not
    // stdlib and carries no PEP 561 `py.typed` marker (i.e. resolution exhausted
    // all six steps without type information).
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
        help: Some(stub_help_text(root_module, import.stub_distribution.as_deref()).into()),
        note: Some(
            "Packages without type stubs or a PEP 561 `py.typed` marker provide no type \
             information — https://peps.python.org/pep-0561/"
                .into(),
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
///
/// Implements [LSPUV-DIAGNOSTICS-MISSING-STUBS]: the typeshed branch is gated
/// on the bundled typeshed index — stub names are never guessed by
/// concatenation.
fn stub_help_text(root_module: &str, distribution: Option<&str>) -> String {
    match distribution {
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
