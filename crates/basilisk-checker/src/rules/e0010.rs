//! BSK-E0010: Import from untyped module.
//!
//! Fires when a module that is not part of the Python standard library or the
//! typing ecosystem is imported.  Third-party packages may lack type stubs,
//! which prevents Basilisk from checking the types of values they produce.

use basilisk_resolver::{ImportInfo, ImportResolution, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0010",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0010",
};

/// Emits BSK-E0010 for imports from modules outside the known stdlib/typing
/// ecosystem.
///
/// Uses the compiled typeshed index from `basilisk-stubs` for O(1) module
/// recognition.  Imports that resolved to workspace or stub files are skipped
/// — they already have type information available.
/// Suppression is handled centrally by the `suppression` module.
pub(crate) struct ImportFromUntypedModule;

impl Rule for ImportFromUntypedModule {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .imports
            .iter()
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| import.resolution == ImportResolution::Unresolved)
            .for_each(|import| diagnostics.push(make_diagnostic(import, &module.path)));
    }
}

fn make_diagnostic(import: &ImportInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Import from `{}` — module may not have type stubs",
            import.module
        ),
        span: import.span,
        path: path.to_owned(),
        help: Some(format!(
            "Install a `{}-stubs` package, add a local stub file, or annotate the import with `# type: ignore`",
            import.module.split('.').next().unwrap_or(&import.module)
        )),
        note: Some(
            "Basilisk requires complete type information for all imported modules".to_owned(),
        ),
    }
}
