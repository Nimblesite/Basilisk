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

// ##########################################################################
// # DELETED BODY — `is_circular_string_annotation`. DO NOT RESTORE IT.
// #
// # It STRIPPED QUOTES OFF SOURCE TEXT by character index and compared what
// # was left to a name:
// #
// #   let content = &ann[1..ann.len() - 1];       // hand-written unquoting
// #   content == attr_name
// #       && !module_scope_names.contains(content)
// #       && !builtin_names.contains(content)
// #
// # A PEP 484 forward reference is a type EXPRESSION, not a bare word:
// # `"list[Foo]"`, `"Foo | None"`, and `" Foo "` all failed the equality
// # test, and the builtin arm was a spelling whitelist
// # (`PYTHON_BUILTIN_TYPE_NAMES`, itself DELETED). `BindingTable::
// # form_of_quoted_annotation` already PARSES a quoted annotation with
// # `ruff_python_parser` and resolves it against the module's final
// # namespace — never inspecting it as text.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
pub(super) fn is_circular_string_annotation(
    _ann: &str,
    _attr_name: &str,
    _module_scope_names: &HashSet<&str>,
    _builtin_names: &HashSet<&str>,
) -> bool {
    panic!(
        "basilisk-checker: `is_circular_string_annotation` was DELETED because it \
         unquoted a forward reference by CHARACTER INDEX and compared the remaining \
         TEXT to a name, with a builtin-spelling whitelist as its escape hatch. It \
         panics because the real implementation — parsing the quoted annotation and \
         resolving it through the binding table, which \
         `BindingTable::form_of_quoted_annotation` already does — DOES NOT EXIST YET. \
         Do not restore the character slicing and do not return `false` in its place."
    )
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

// ##########################################################################
// # DELETED AND GONE — `PYTHON_BUILTIN_TYPE_NAMES`, a whitelist of builtin
// # type SPELLINGS. NO SHELL: a `&[&str]` table cannot panic, and there is
// # nothing to keep visible but its readers. DO NOT RECREATE IT.
// #
// # It answered "is this annotation naming a builtin type?" by string
// # membership, so `from builtins import int as Int` was not a builtin and a
// # module-level `class int: ...` still was. CLAUDE.md: builtins are not an
// # exception. `BindingTable::form_of_with_builtins` already resolves a bare
// # name to its `builtins` definition ONLY while the module has not rebound
// # it — the exact question this table got wrong in both directions.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
