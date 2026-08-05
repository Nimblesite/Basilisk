//! Implements [CHKARCH-CONFORMANCE-MODE] — the symbol-naming ban. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE-MODE and
//! docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md
//!
//! Basilisk must never identify a Python typing symbol by writing that symbol's
//! NAME into Rust. A name in Python source is not the symbol:
//! `from typing import TypeVar as TV` binds a `TypeVar` under a different name,
//! and a local `class TypeVar:` is not one at all. Matching characters answers
//! neither case correctly — and matching them against the vocabulary of the
//! python/typing conformance fixtures made the score a measurement of how well
//! the code had been fitted to those fixtures rather than of whether the
//! checker works. ~431 such sites were deleted in the 2026-08 integrity audit.
//!
//! This test is the ratchet that keeps them deleted. It fails the build if any
//! import-requiring Python symbol name appears as a string literal in
//! production Rust, in ANY form — a comparison, a match arm, a const array, or
//! an argument to a helper that takes the name as a parameter. Passing the name
//! through an API does not launder it; the checker would still only work
//! because a human typed the fixtures' vocabulary into the source.
//!
//! Recognition must instead resolve a use-site expression through the module's
//! imports to the declaration it binds to, and derive meaning from that
//! declaration.
//!
//! NEVER weaken this test, add an allowlist entry to make a diagnostic fire, or
//! mark it `#[ignore]`. Making the checker pass by re-teaching it the fixtures'
//! vocabulary is the exact defect this exists to prevent.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Names that Python code must IMPORT to use. Writing one of these into Rust to
/// decide what an expression means is the banned mechanism.
///
/// This list is the ban's subject matter, not a recognition table: nothing in
/// the checker may read it. It lives only here, in a test that forbids its use.
const IMPORT_REQUIRING_NAMES: &[&str] = &[
    // typing / typing_extensions special forms
    "TypeVar",
    "TypeVarTuple",
    "ParamSpec",
    "TypedDict",
    "TypeAliasType",
    "NewType",
    "NamedTuple",
    "Protocol",
    "Generic",
    "Final",
    "ClassVar",
    "Annotated",
    "Unpack",
    "Union",
    "Optional",
    "TypeAlias",
    "Required",
    "NotRequired",
    "ReadOnly",
    "Literal",
    "LiteralString",
    "Self",
    "Never",
    "NoReturn",
    "Concatenate",
    "TypeGuard",
    "TypeIs",
    "TypeForm",
    "assert_type",
    "reveal_type",
    "runtime_checkable",
    "dataclass_transform",
    "no_type_check",
    "TYPE_CHECKING",
    // collections.abc / typing aliases
    "Callable",
    "Iterable",
    "Iterator",
    "Generator",
    "AsyncGenerator",
    "AsyncIterator",
    "AsyncIterable",
    "Sequence",
    "MutableSequence",
    "Mapping",
    "MutableMapping",
    "Collection",
    "Container",
    "Hashable",
    "Sized",
    "Awaitable",
    "Coroutine",
    "AbstractSet",
    "Deque",
    // typing's deprecated capitalised builtin aliases
    "List",
    "Dict",
    "FrozenSet",
    "Tuple",
    // dataclasses
    "dataclass",
    "InitVar",
    "KW_ONLY",
    // enum
    "IntEnum",
    "StrEnum",
    "IntFlag",
    "ReprEnum",
    "nonmember",
    // abc / functools / warnings
    "abstractmethod",
    "total_ordering",
    "deprecated",
];

/// Files exempt from the ban, and why. Each entry is a mechanism that does not
/// decide meaning from a name.
///
/// Adding an entry to make a diagnostic fire again is forbidden — that is the
/// banned mechanism returning under a different filename.
const EXEMPT: &[(&str, &str)] = &[(
    "tests/no_symbol_naming.rs",
    "this test — it names the banned symbols in order to forbid them",
)];

/// A banned occurrence.
#[derive(Debug)]
struct Finding {
    file: PathBuf,
    line: usize,
    text: String,
    name: &'static str,
}

#[test]
fn no_production_rust_names_an_import_requiring_python_symbol() {
    let workspace = workspace_root();
    let mut findings: Vec<Finding> = Vec::new();

    for crate_name in ["basilisk-checker", "basilisk-resolver"] {
        let src = workspace.join("crates").join(crate_name).join("src");
        collect_rust_files(&src, &mut |file| scan_file(&workspace, file, &mut findings));
    }

    assert!(
        findings.is_empty(),
        "The symbol-naming ban is violated in {} place(s).\n\n\
         Identifying a Python typing symbol by writing its NAME into Rust is banned \
         permanently — as a comparison, a match arm, a const array, OR an argument to a \
         helper that takes the name as a parameter. See the ban in CLAUDE.md and \
         docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md.\n\n\
         Recognition must resolve the expression through the module's imports to the \
         declaration it binds to, and read meaning from that declaration — never from the \
         characters at the use site.\n\n\
         Do NOT fix this by adding an EXEMPT entry.\n\n{}",
        findings.len(),
        render(&findings)
    );
}

/// Scan one file for banned names outside comments and test modules.
fn scan_file(workspace: &Path, file: &Path, findings: &mut Vec<Finding>) {
    let relative = file.strip_prefix(workspace).unwrap_or(file);
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    if EXEMPT
        .iter()
        .any(|(exempt, _)| relative_text.ends_with(exempt))
    {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(file) else {
        return;
    };
    // Test modules are excluded: a test may legitimately write Python source
    // containing these names as FIXTURE INPUT.
    let production = contents
        .split_once("#[cfg(test)]")
        .map_or(contents.as_str(), |(before, _)| before);

    for (index, line) in production.lines().enumerate() {
        let code = strip_comment(line);
        if code.trim().is_empty() {
            continue;
        }
        for name in IMPORT_REQUIRING_NAMES {
            if names_symbol(code, name) {
                findings.push(Finding {
                    file: relative.to_path_buf(),
                    line: index + 1,
                    text: line.trim().to_owned(),
                    name,
                });
            }
        }
    }
}

/// Does this line of code contain `name` as a string literal?
///
/// Matches the bare name (`"TypeVar"`) and the subscript/call prefixes the old
/// text-matching code used (`"TypeVar["`, `"TypeVar("`), plus dotted spellings
/// (`"typing.TypeVar"`).
fn names_symbol(code: &str, name: &str) -> bool {
    [
        format!("\"{name}\""),
        format!("\"{name}[\""),
        format!("\"{name}(\""),
        format!(".{name}\""),
    ]
    .iter()
    .any(|needle| code.contains(needle.as_str()))
}

/// Drop a trailing `//` comment, ignoring `//` inside string literals. Prose
/// that mentions a banned name is fine — only code is scanned.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' && in_string {
            escaped = true;
        } else if byte == b'"' {
            in_string = !in_string;
        } else if !in_string && byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return line.get(..index).unwrap_or("");
        }
        index += 1;
    }
    line
}

/// Every `.rs` file under `dir`, recursively.
fn collect_rust_files(dir: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rust_files(&path, visit);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            visit(&path);
        }
    }
}

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Group findings by file for a readable failure.
fn render(findings: &[Finding]) -> String {
    let files: BTreeSet<&Path> = findings.iter().map(|f| f.file.as_path()).collect();
    let mut out = String::new();
    for file in files {
        out.push_str(&format!("{}\n", file.display()));
        for finding in findings.iter().filter(|f| f.file == file) {
            out.push_str(&format!(
                "  {:>5}: names `{}` — {}\n",
                finding.line, finding.name, finding.text
            ));
        }
    }
    out
}
