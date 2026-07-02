//! Implements [ANALYSIS-GRAPH]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-GRAPH
//!
//! Import dependency graph — directed graph of file-to-file import edges.
//!
//! Built from resolved `ImportInfo.resolved_path` fields after workspace
//! scanning. Serves **navigation reverse-lookups** ("who imports this file?")
//! for cross-file references and rename; incremental *invalidation* is the
//! salsa engine's job ([CHKARCH-INCREMENTAL-SALSA]), which tracks cross-file
//! dependencies content-precisely rather than at file granularity.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Directed import dependency graph.
///
/// Edges go from importer → imported (i.e., if `main.py` imports `utils.py`,
/// there is an edge `main.py → utils.py`).
// Implements [ANALYSIS-GRAPH-STRUCT] — forward/reverse adjacency maps; spec's
// HashMap<PathBuf, HashSet<PathBuf>> shape.
#[derive(Debug, Default)]
pub struct ImportGraph {
    /// Forward edges: file → set of files it imports.
    forward: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Reverse edges: file → set of files that import it.
    reverse: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl ImportGraph {
    /// Create a new empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an import edge: `importer` imports `imported`.
    pub fn add_edge(&mut self, importer: &Path, dependency: &Path) {
        let _ = self
            .forward
            .entry(importer.to_path_buf())
            .or_default()
            .insert(dependency.to_path_buf());
        let _ = self
            .reverse
            .entry(dependency.to_path_buf())
            .or_default()
            .insert(importer.to_path_buf());
        // Ensure both nodes exist in both maps for consistent traversal.
        let _ = self.forward.entry(dependency.to_path_buf()).or_default();
        let _ = self.reverse.entry(importer.to_path_buf()).or_default();
    }

    /// Register a file in the graph with no edges (leaf node).
    pub fn add_node(&mut self, path: &Path) {
        let _ = self.forward.entry(path.to_path_buf()).or_default();
        let _ = self.reverse.entry(path.to_path_buf()).or_default();
    }

    /// Get the files that import a given file (reverse dependencies / importers).
    ///
    /// The navigation handlers use this to search importers for cross-file
    /// references and rename edits. Implements [ANALYSIS-CROSSLSP-REFS].
    #[must_use]
    pub fn importers_of(&self, path: &Path) -> Vec<PathBuf> {
        self.reverse
            .get(path)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Build the import graph from the workspace index.
    ///
    /// Walks all files in the index, extracts `ImportInfo.resolved_path` from
    /// each `ResolvedModule`, and adds edges to the graph.
    // Implements [ANALYSIS-GRAPH-BUILD] — walks `ImportInfo.resolved_path` for
    // every indexed file, populating forward and reverse edges.
    pub fn build_from_index(&mut self, index: &crate::workspace::WorkspaceIndex) {
        for entry in &index.files {
            let importer_path = entry.key().clone();
            self.add_node(&importer_path);

            if let Some(resolved) = &entry.resolved {
                for import in &resolved.imports {
                    if let Some(imported_path) = &import.resolved_path {
                        self.add_edge(&importer_path, imported_path);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_has_no_importers() {
        let graph = ImportGraph::new();
        assert!(graph
            .importers_of(&PathBuf::from("/project/main.py"))
            .is_empty());
    }

    #[test]
    fn add_edge_creates_reverse_lookup() {
        let mut graph = ImportGraph::new();
        let main = PathBuf::from("/project/main.py");
        let utils = PathBuf::from("/project/utils.py");
        graph.add_edge(&main, &utils);

        assert_eq!(graph.importers_of(&utils), vec![main.clone()]);
        assert!(
            graph.importers_of(&main).is_empty(),
            "nothing imports main.py"
        );
    }

    #[test]
    fn importers_of_returns_direct_importers_only() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        let c = PathBuf::from("/c.py");
        // a imports b, b imports c: only b directly imports c.
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);

        assert_eq!(graph.importers_of(&c), vec![b.clone()]);
    }

    #[test]
    fn isolated_node_has_no_importers() {
        let mut graph = ImportGraph::new();
        let solo = PathBuf::from("/solo.py");
        graph.add_node(&solo);
        assert!(graph.importers_of(&solo).is_empty());
    }
}
