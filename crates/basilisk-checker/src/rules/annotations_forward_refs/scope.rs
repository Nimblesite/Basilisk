//! Implements [`annotations_forward_refs`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! Module scope and circular annotation helpers for `annotations_forward_refs`.
//!
//! Contains helpers for building module scope name sets, detecting circular
//! string annotations, and the canonical list of Python built-in type names.

use std::collections::HashSet;

use basilisk_resolver::{ImportKind, ResolvedModule};

// ---------------------------------------------------------------------------
// Circular annotation detection
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation is a string literal that circularly references
/// its own attribute name, and that name is not otherwise defined.
pub(super) fn is_circular_string_annotation(
    ann: &str,
    attr_name: &str,
    module_scope_names: &HashSet<&str>,
    builtin_names: &HashSet<&str>,
) -> bool {
    let content = if (ann.starts_with('"') && ann.ends_with('"') && ann.len() >= 2)
        || (ann.starts_with('\'') && ann.ends_with('\'') && ann.len() >= 2)
    {
        &ann[1..ann.len() - 1]
    } else {
        return false;
    };
    content == attr_name
        && !module_scope_names.contains(content)
        && !builtin_names.contains(content)
}

// ---------------------------------------------------------------------------
// Module scope helpers
// ---------------------------------------------------------------------------

/// Build a set of names defined at module scope (classes, vars, imports).
pub(super) fn build_module_scope_names<'a>(module: &'a ResolvedModule) -> HashSet<&'a str> {
    let mut names: HashSet<&'a str> = HashSet::new();
    for cls in &module.classes {
        let _ = names.insert(cls.name.as_str());
    }
    for var in &module.module_vars {
        let _ = names.insert(var.name.as_str());
    }
    for imp in &module.imports {
        match imp.kind {
            ImportKind::From => {
                for name in &imp.names {
                    let _ = names.insert(name.as_str());
                }
            }
            ImportKind::Plain => {
                if let Some(name) = imp.module.split('.').next() {
                    let _ = names.insert(name);
                }
            }
            ImportKind::Star => {}
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Python built-in type names
// ---------------------------------------------------------------------------

/// Python built-in type names that are always valid as forward references in annotations.
pub(super) const PYTHON_BUILTIN_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "complex",
    "bytearray",
    "memoryview",
    "object",
    "type",
    "None",
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "range",
    "slice",
    "super",
    "classmethod",
    "staticmethod",
    "property",
    "Exception",
    "BaseException",
    "ValueError",
    "TypeError",
    "AttributeError",
    "KeyError",
    "IndexError",
    "RuntimeError",
    "StopIteration",
    "NotImplementedError",
    "OSError",
    "IOError",
    "FileNotFoundError",
    "PermissionError",
    "TimeoutError",
    "ConnectionError",
    "ArithmeticError",
    "OverflowError",
    "ZeroDivisionError",
    "ImportError",
    "ModuleNotFoundError",
    "NameError",
    "UnboundLocalError",
    "LookupError",
    "SyntaxError",
    "SystemExit",
    "KeyboardInterrupt",
    "GeneratorExit",
    "UnicodeError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
];
