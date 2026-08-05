//! Implements [`qualifiers_annotated`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! Helper functions for `qualifiers_annotated`: name collection for the
//! undefined-bare-name check. Type-expression validity itself is judged
//! structurally by the shared judge ([LINESCANPLAN-AST-MIGRATION]).

use std::collections::HashSet;

use basilisk_resolver::{ClassInfo, FunctionInfo, ImportInfo, ImportKind, Span, VariableInfo};

use crate::span_util::slice_span;

/// Slice a source string to get the text at a given span (display only —
/// never a verdict input).
pub(super) fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

/// Python built-in names that require no import and are always in scope.
const BUILTIN_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "bytearray",
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "type",
    "object",
    "None",
    "complex",
    "memoryview",
    "range",
    "slice",
    "Exception",
    "BaseException",
    "ValueError",
    "TypeError",
    "KeyError",
    "IndexError",
    "AttributeError",
    "RuntimeError",
    "StopIteration",
    "NotImplementedError",
    "OverflowError",
    "ZeroDivisionError",
    "NameError",
    "ImportError",
    "OSError",
    "IOError",
    "FileNotFoundError",
    "PermissionError",
    "TimeoutError",
];

/// Collect all names that are defined in module scope.
///
/// Returns a set of names that can be used as valid references in annotations.
/// This includes:
/// - Module-level variable names (including `TypeVar`, `TypeAlias`, etc.)
/// - Names imported via `from X import Y` or `import X`
/// - Class names
/// - Function names
pub(super) fn collect_defined_names(
    vars: &[VariableInfo],
    imports: &[ImportInfo],
    classes: &[ClassInfo],
    functions: &[FunctionInfo],
) -> HashSet<String> {
    let mut names: HashSet<String> = BUILTIN_TYPE_NAMES
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    for var in vars {
        let _ = names.insert(var.name.clone());
    }

    for import in imports {
        match import.kind {
            ImportKind::Plain => {
                // `import os` binds `os`
                if let Some(first) = import.module.split('.').next() {
                    let _ = names.insert(first.to_owned());
                }
            }
            ImportKind::From => {
                // `from typing import Annotated` binds `Annotated`
                for name in &import.names {
                    let _ = names.insert(name.clone());
                }
            }
            ImportKind::Star => {
                // `from typing import *` — we can't know what's imported, so skip
            }
        }
    }

    for cls in classes {
        let _ = names.insert(cls.name.clone());
    }

    for func in functions {
        let _ = names.insert(func.name.clone());
    }

    names
}
