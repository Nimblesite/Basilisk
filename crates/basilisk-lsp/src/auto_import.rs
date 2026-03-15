//! Auto-import symbol index for workspace-wide import suggestions.
//!
//! Builds an index of all exported symbols from workspace files so the
//! completion handler can suggest imports for unknown symbols. When a
//! user types a symbol name that isn't in scope, auto-import suggests
//! adding `from module import symbol` at the top of the file.
//!
//! # Architecture
//!
//! - [`SymbolIndex`] maps symbol names to their source locations.
//! - [`build_symbol_index`] walks every file in a [`WorkspaceIndex`] and
//!   collects exported functions, classes, and module-level variables.
//! - [`suggest_imports`] looks up a symbol name and returns candidate imports.
//! - [`generate_import_edit`] creates an LSP `TextEdit` to insert the import.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::workspace::WorkspaceIndex;

/// The kind of exported symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    /// A function definition.
    Function,
    /// A class definition.
    Class,
    /// A module-level variable or constant.
    Variable,
}

/// A single exported symbol from a workspace file.
#[derive(Debug, Clone)]
pub struct ExportedSymbol {
    /// The symbol name (e.g. `"MyClass"`, `"parse_file"`).
    pub name: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// The module path relative to workspace root (e.g. `"basilisk_parser.parse"`).
    pub module_path: String,
    /// The filesystem path of the source file.
    pub file_path: PathBuf,
}

/// Index of all exported symbols in the workspace.
///
/// Maps symbol names (lowercased for case-insensitive lookup) to a list of
/// candidates. Multiple files may export the same symbol name.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// Maps lowercased symbol name → list of exported symbols.
    symbols: HashMap<String, Vec<ExportedSymbol>>,
    /// Total number of indexed symbols.
    count: usize,
}

impl SymbolIndex {
    /// Look up candidates for a symbol name (case-insensitive).
    #[must_use]
    pub fn lookup(&self, name: &str) -> &[ExportedSymbol] {
        self.symbols
            .get(&name.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }

    /// Look up candidates for a symbol name (exact case match).
    #[must_use]
    pub fn lookup_exact(&self, name: &str) -> Vec<&ExportedSymbol> {
        self.symbols
            .get(&name.to_ascii_lowercase())
            .map_or_else(Vec::new, |candidates| {
                candidates.iter().filter(|s| s.name == name).collect()
            })
    }

    /// Total number of indexed symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Build a symbol index from all files in the workspace.
///
/// Iterates every file in the workspace index, extracts exported functions,
/// classes, and module-level variables, and indexes them by name.
#[must_use]
pub fn build_symbol_index(index: &WorkspaceIndex) -> SymbolIndex {
    let mut symbols: HashMap<String, Vec<ExportedSymbol>> = HashMap::new();
    let mut count = 0usize;

    for entry in &index.files {
        let file_path = entry.key();
        let Some(resolved) = &entry.value().resolved else {
            continue;
        };

        let module_path = derive_module_path(file_path, &index.roots);

        for func in &resolved.functions {
            // Skip private symbols (leading underscore) and methods (class_name set).
            if func.name.starts_with('_') || func.class_name.is_some() {
                continue;
            }
            let sym = ExportedSymbol {
                name: func.name.clone(),
                kind: SymbolKind::Function,
                module_path: module_path.clone(),
                file_path: file_path.clone(),
            };
            symbols
                .entry(func.name.to_ascii_lowercase())
                .or_default()
                .push(sym);
            count += 1;
        }

        for class in &resolved.classes {
            if class.name.starts_with('_') {
                continue;
            }
            let sym = ExportedSymbol {
                name: class.name.clone(),
                kind: SymbolKind::Class,
                module_path: module_path.clone(),
                file_path: file_path.clone(),
            };
            symbols
                .entry(class.name.to_ascii_lowercase())
                .or_default()
                .push(sym);
            count += 1;
        }

        for var in &resolved.module_vars {
            // Only index ALL_CAPS constants and public names.
            if var.name.starts_with('_') {
                continue;
            }
            let sym = ExportedSymbol {
                name: var.name.clone(),
                kind: SymbolKind::Variable,
                module_path: module_path.clone(),
                file_path: file_path.clone(),
            };
            symbols
                .entry(var.name.to_ascii_lowercase())
                .or_default()
                .push(sym);
            count += 1;
        }
    }

    SymbolIndex { symbols, count }
}

/// Suggest import candidates for an unresolved symbol name.
///
/// Returns a list of possible imports, sorted by relevance (exact name match
/// first, then by module path length as a proxy for specificity).
#[must_use]
pub fn suggest_imports<'a>(index: &'a SymbolIndex, name: &str) -> Vec<&'a ExportedSymbol> {
    let mut candidates = index.lookup_exact(name);
    // Sort by module path length (shorter = more likely the right one).
    candidates.sort_by_key(|s| s.module_path.len());
    candidates
}

/// Generate the import statement text for a symbol.
///
/// Returns a string like `"from module.path import SymbolName\n"`.
#[must_use]
pub fn generate_import_text(symbol: &ExportedSymbol) -> String {
    format!("from {} import {}\n", symbol.module_path, symbol.name)
}

/// Find the byte offset where a new import should be inserted.
///
/// Scans the source for the last existing import statement and returns the
/// byte offset of the line after it. If no imports exist, returns 0.
#[must_use]
pub fn find_import_insertion_offset(source: &str) -> usize {
    let mut last_import_end = 0usize;
    let mut in_import = false;

    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_start = source.lines().take(idx).map(|l| l.len() + 1).sum::<usize>();
        let line_end = line_start + line.len() + 1;

        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            last_import_end = line_end.min(source.len());
            in_import = true;
        } else if in_import && trimmed.is_empty() {
            // Blank line after imports — good insertion point.
            last_import_end = line_end.min(source.len());
            break;
        } else if in_import && !trimmed.starts_with('#') {
            // First non-import, non-blank, non-comment line — insert before this.
            break;
        }
    }

    last_import_end
}

/// Derive a Python module path from a filesystem path relative to workspace roots.
///
/// Converts `/workspace/src/mypackage/utils.py` → `mypackage.utils`
/// Strips `__init__.py` to get the package name.
fn derive_module_path(file_path: &Path, roots: &[PathBuf]) -> String {
    // Find the workspace root that contains this file.
    let relative = roots
        .iter()
        .filter_map(|root| file_path.strip_prefix(root).ok())
        .min_by_key(|p| p.components().count());

    let Some(rel) = relative else {
        // Fallback: use the file stem.
        return file_path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    };

    // Convert path components to dotted module path.
    let mut parts: Vec<&str> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Strip .py/.pyi extension from last component.
    if let Some(last) = parts.last_mut() {
        if let Some(stem) = last.strip_suffix(".py") {
            *last = stem;
        } else if let Some(stem) = last.strip_suffix(".pyi") {
            *last = stem;
        }
    }

    // Remove __init__ (package init files represent the package itself).
    if parts.last() == Some(&"__init__") {
        let _ = parts.pop();
    }

    parts.join(".")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn derive_module_path_simple() {
        let roots = vec![PathBuf::from("/workspace/src")];
        let path = PathBuf::from("/workspace/src/mypackage/utils.py");
        assert_eq!(derive_module_path(&path, &roots), "mypackage.utils");
    }

    #[test]
    fn derive_module_path_init() {
        let roots = vec![PathBuf::from("/workspace/src")];
        let path = PathBuf::from("/workspace/src/mypackage/__init__.py");
        assert_eq!(derive_module_path(&path, &roots), "mypackage");
    }

    #[test]
    fn derive_module_path_top_level() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/main.py");
        assert_eq!(derive_module_path(&path, &roots), "main");
    }

    #[test]
    fn find_import_insertion_after_imports() {
        let source = "import os\nfrom sys import path\n\nx = 1\n";
        let offset = find_import_insertion_offset(source);
        // Should be after the blank line following imports.
        assert!(offset > 0);
        assert!(offset <= source.len());
    }

    #[test]
    fn find_import_insertion_no_imports() {
        let source = "x = 1\ny = 2\n";
        let offset = find_import_insertion_offset(source);
        assert_eq!(offset, 0);
    }

    #[test]
    fn generate_import_text_function() {
        let sym = ExportedSymbol {
            name: "parse_file".to_owned(),
            kind: SymbolKind::Function,
            module_path: "basilisk_parser".to_owned(),
            file_path: PathBuf::from("/workspace/src/basilisk_parser.py"),
        };
        assert_eq!(
            generate_import_text(&sym),
            "from basilisk_parser import parse_file\n"
        );
    }

    #[test]
    fn symbol_index_lookup() {
        let mut symbols = HashMap::new();
        let sym = ExportedSymbol {
            name: "MyClass".to_owned(),
            kind: SymbolKind::Class,
            module_path: "mypackage.models".to_owned(),
            file_path: PathBuf::from("/workspace/src/mypackage/models.py"),
        };
        symbols
            .entry("myclass".to_owned())
            .or_insert_with(Vec::new)
            .push(sym);
        let index = SymbolIndex { symbols, count: 1 };

        assert_eq!(index.lookup("MyClass").len(), 1);
        assert_eq!(index.lookup("myclass").len(), 1);
        assert_eq!(index.lookup("NonExistent").len(), 0);
        assert_eq!(index.lookup_exact("MyClass").len(), 1);
        assert_eq!(index.lookup_exact("myclass").len(), 0);
    }
}
