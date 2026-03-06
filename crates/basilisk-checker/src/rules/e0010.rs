//! BSK-E0010: Import from untyped module.
//!
//! Fires when a module that is not part of the Python standard library or the
//! typing ecosystem is imported.  Third-party packages may lack type stubs,
//! which prevents Basilisk from checking the types of values they produce.

use basilisk_resolver::{ImportInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0010",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0010",
};

/// Known-safe standard-library and typing root module names.
///
/// Any import whose root (first dotted component) matches one of these strings
/// is considered to have complete type information and will not trigger E0010.
const STDLIB_ROOTS: &[&str] = &[
    "__future__",
    "abc",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "base64",
    "basilisk",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "codecs",
    "collections",
    "contextlib",
    "copy",
    "cmath",
    "csv",
    "ctypes",
    "dataclasses",
    "datetime",
    "decimal",
    "difflib",
    "dis",
    "email",
    "enum",
    "fnmatch",
    "fractions",
    "functools",
    "gc",
    "glob",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "importlib",
    "inspect",
    "io",
    "itertools",
    "json",
    "logging",
    "lzma",
    "math",
    "multiprocessing",
    "numbers",
    "operator",
    "os",
    "pathlib",
    "pickle",
    "pkgutil",
    "platform",
    "pprint",
    "queue",
    "random",
    "re",
    "secrets",
    "shutil",
    "signal",
    "socket",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "struct",
    "sys",
    "tarfile",
    "tempfile",
    "textwrap",
    "threading",
    "time",
    "traceback",
    "tracemalloc",
    "types",
    "typing",
    "typing_extensions",
    "unittest",
    "urllib",
    "uuid",
    "warnings",
    "weakref",
    "xml",
    "zipfile",
    "zlib",
];

/// Emits BSK-E0010 for imports from modules outside the known stdlib/typing
/// ecosystem.
///
/// Suppression is handled centrally by the `suppression` module — this rule
/// does not filter diagnostics itself.
pub(crate) struct ImportFromUntypedModule;

impl Rule for ImportFromUntypedModule {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .imports
            .iter()
            .filter(|import| !is_stdlib(&import.module))
            .for_each(|import| diagnostics.push(make_diagnostic(import, &module.path)));
    }
}

/// Returns `true` when the module root is a known stdlib or typing package.
fn is_stdlib(module_name: &str) -> bool {
    let root = module_name.split('.').next().unwrap_or(module_name);
    STDLIB_ROOTS.contains(&root)
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
