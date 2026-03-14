//! Import dependency graph — directed graph of file-to-file import edges.
//!
//! Built from resolved `ImportInfo.resolved_path` fields after workspace scanning.
//! Supports topological ordering for analysis, cycle detection, and reverse
//! dependency lookups for incremental invalidation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Directed import dependency graph.
///
/// Edges go from importer → imported (i.e., if `main.py` imports `utils.py`,
/// there is an edge `main.py → utils.py`).
#[derive(Debug, Default)]
pub struct ImportGraph {
    /// Forward edges: file → set of files it imports.
    forward: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Reverse edges: file → set of files that import it.
    reverse: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Files involved in circular import chains.
    cycles: Vec<ImportCycle>,
}

/// A circular import chain detected in the graph.
#[derive(Debug, Clone)]
pub struct ImportCycle {
    /// The files forming the cycle, in import order.
    pub chain: Vec<PathBuf>,
}

impl ImportGraph {
    /// Create a new empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an import edge: `importer` imports `imported`.
    pub fn add_edge(&mut self, importer: &Path, imported: &Path) {
        let _ = self
            .forward
            .entry(importer.to_path_buf())
            .or_default()
            .insert(imported.to_path_buf());
        let _ = self
            .reverse
            .entry(imported.to_path_buf())
            .or_default()
            .insert(importer.to_path_buf());
        // Ensure both nodes exist in both maps for consistent traversal.
        let _ = self.forward.entry(imported.to_path_buf()).or_default();
        let _ = self.reverse.entry(importer.to_path_buf()).or_default();
    }

    /// Register a file in the graph with no edges (leaf node).
    pub fn add_node(&mut self, path: &Path) {
        let _ = self.forward.entry(path.to_path_buf()).or_default();
        let _ = self.reverse.entry(path.to_path_buf()).or_default();
    }

    /// Get the files that a given file imports (forward dependencies).
    #[must_use]
    pub fn imports_of(&self, path: &Path) -> Vec<PathBuf> {
        self.forward
            .get(path)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the files that import a given file (reverse dependencies / importers).
    #[must_use]
    pub fn importers_of(&self, path: &Path) -> Vec<PathBuf> {
        self.reverse
            .get(path)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all transitive importers of a file (files that would need re-analysis
    /// if this file changes).
    #[must_use]
    pub fn transitive_importers(&self, path: &Path) -> Vec<PathBuf> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(path.to_path_buf());
        let _ = visited.insert(path.to_path_buf());

        while let Some(current) = queue.pop_front() {
            if let Some(importers) = self.reverse.get(&current) {
                for importer in importers {
                    if visited.insert(importer.clone()) {
                        queue.push_back(importer.clone());
                    }
                }
            }
        }

        // Remove the original file from the result.
        let _ = visited.remove(path);
        visited.into_iter().collect()
    }

    /// Total number of files in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.forward.len()
    }

    /// Total number of import edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.forward.values().map(HashSet::len).sum()
    }

    /// Topological sort of all files in the graph.
    ///
    /// Returns files ordered so that imported modules appear before their
    /// importers. Files in cycles are placed after their non-cyclic dependencies
    /// using Kahn's algorithm.
    #[must_use]
    pub fn topological_order(&self) -> Vec<PathBuf> {
        let mut in_degree: HashMap<PathBuf, usize> = HashMap::new();
        for (node, deps) in &self.forward {
            let _ = in_degree.entry(node.clone()).or_insert(0);
            for dep in deps {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm: start with nodes that have no incoming edges
        // (i.e., no one imports them — they are leaf dependencies).
        // NOTE: We use reverse edges for in-degree since we want imported-first order.
        let mut actual_in_degree: HashMap<PathBuf, usize> = HashMap::new();
        for node in self.forward.keys() {
            let count = self.forward.get(node).map(HashSet::len).unwrap_or_default();
            let _ = actual_in_degree.insert(node.clone(), count);
        }

        let mut queue: VecDeque<PathBuf> = actual_in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(node, _)| node.clone())
            .collect();

        let mut result = Vec::with_capacity(self.forward.len());

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            if let Some(importers) = self.reverse.get(&node) {
                for importer in importers {
                    if let Some(deg) = actual_in_degree.get_mut(importer) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(importer.clone());
                        }
                    }
                }
            }
        }

        // Any remaining nodes are in cycles — add them at the end.
        for node in self.forward.keys() {
            if !result.contains(node) {
                result.push(node.clone());
            }
        }

        result
    }

    /// Detect circular import chains in the graph.
    ///
    /// Uses DFS with coloring (white/gray/black) to find back edges.
    /// Returns a list of detected cycles.
    pub fn detect_cycles(&mut self) -> &[ImportCycle] {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Color {
            White,
            Gray,
            Black,
        }

        let mut colors: HashMap<PathBuf, Color> = self
            .forward
            .keys()
            .map(|k| (k.clone(), Color::White))
            .collect();
        let mut path_stack: Vec<PathBuf> = Vec::new();
        let mut cycles: Vec<ImportCycle> = Vec::new();

        fn dfs(
            node: &Path,
            forward: &HashMap<PathBuf, HashSet<PathBuf>>,
            colors: &mut HashMap<PathBuf, Color>,
            path_stack: &mut Vec<PathBuf>,
            cycles: &mut Vec<ImportCycle>,
        ) {
            if let Some(color) = colors.get_mut(&node.to_path_buf()) {
                *color = Color::Gray;
            }
            path_stack.push(node.to_path_buf());

            if let Some(deps) = forward.get(node) {
                for dep in deps {
                    match colors.get(dep).copied() {
                        Some(Color::White) => {
                            dfs(dep, forward, colors, path_stack, cycles);
                        }
                        Some(Color::Gray) => {
                            // Back edge — cycle found. Extract the cycle from path_stack.
                            if let Some(cycle_start) = path_stack.iter().position(|p| p == dep) {
                                let chain: Vec<PathBuf> = path_stack[cycle_start..].to_vec();
                                cycles.push(ImportCycle { chain });
                            }
                        }
                        Some(Color::Black) | None => {}
                    }
                }
            }

            let _ = path_stack.pop();
            if let Some(color) = colors.get_mut(&node.to_path_buf()) {
                *color = Color::Black;
            }
        }

        let nodes: Vec<PathBuf> = self.forward.keys().cloned().collect();
        for node in &nodes {
            if colors.get(node).copied() == Some(Color::White) {
                dfs(
                    node,
                    &self.forward,
                    &mut colors,
                    &mut path_stack,
                    &mut cycles,
                );
            }
        }

        self.cycles = cycles;
        &self.cycles
    }

    /// Get previously detected cycles.
    #[must_use]
    pub fn cycles(&self) -> &[ImportCycle] {
        &self.cycles
    }

    /// Build the import graph from the workspace index.
    ///
    /// Walks all files in the index, extracts `ImportInfo.resolved_path` from
    /// each `ResolvedModule`, and adds edges to the graph.
    pub fn build_from_index(&mut self, index: &crate::workspace::WorkspaceIndex) {
        for entry in index.files.iter() {
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
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let graph = ImportGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn add_edge_creates_forward_and_reverse() {
        let mut graph = ImportGraph::new();
        let main = PathBuf::from("/project/main.py");
        let utils = PathBuf::from("/project/utils.py");
        graph.add_edge(&main, &utils);

        assert_eq!(graph.imports_of(&main), vec![utils.clone()]);
        assert_eq!(graph.importers_of(&utils), vec![main.clone()]);
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn topological_order_simple_chain() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        let c = PathBuf::from("/c.py");
        // a imports b, b imports c
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);

        let order = graph.topological_order();
        let pos_a = order.iter().position(|p| p == &a).unwrap();
        let pos_b = order.iter().position(|p| p == &b).unwrap();
        let pos_c = order.iter().position(|p| p == &c).unwrap();

        // c should come before b, b before a (imported-first)
        assert!(pos_c < pos_b, "c must come before b");
        assert!(pos_b < pos_a, "b must come before a");
    }

    #[test]
    fn topological_order_diamond() {
        let mut graph = ImportGraph::new();
        let main = PathBuf::from("/main.py");
        let left = PathBuf::from("/left.py");
        let right = PathBuf::from("/right.py");
        let base = PathBuf::from("/base.py");

        graph.add_edge(&main, &left);
        graph.add_edge(&main, &right);
        graph.add_edge(&left, &base);
        graph.add_edge(&right, &base);

        let order = graph.topological_order();
        let pos_main = order.iter().position(|p| p == &main).unwrap();
        let pos_base = order.iter().position(|p| p == &base).unwrap();

        assert!(pos_base < pos_main, "base must come before main");
    }

    #[test]
    fn cycle_detection() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &a);

        let cycles = graph.detect_cycles();
        assert!(!cycles.is_empty(), "should detect circular import");
    }

    #[test]
    fn no_cycles_in_dag() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        let c = PathBuf::from("/c.py");
        graph.add_edge(&a, &b);
        graph.add_edge(&a, &c);

        let cycles = graph.detect_cycles();
        assert!(cycles.is_empty(), "DAG should have no cycles");
    }

    #[test]
    fn transitive_importers() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        let c = PathBuf::from("/c.py");
        let d = PathBuf::from("/d.py");

        // d imports c, c imports b, a imports b
        graph.add_edge(&d, &c);
        graph.add_edge(&c, &b);
        graph.add_edge(&a, &b);

        let importers = graph.transitive_importers(&b);
        // a, c, d all transitively depend on b
        assert!(importers.contains(&a));
        assert!(importers.contains(&c));
        assert!(importers.contains(&d));
        assert!(!importers.contains(&b));
    }

    #[test]
    fn leaf_node_has_no_importers() {
        let mut graph = ImportGraph::new();
        let a = PathBuf::from("/a.py");
        let b = PathBuf::from("/b.py");
        graph.add_edge(&a, &b);

        let importers = graph.transitive_importers(&a);
        assert!(importers.is_empty());
    }

    #[test]
    fn isolated_node() {
        let mut graph = ImportGraph::new();
        let solo = PathBuf::from("/solo.py");
        graph.add_node(&solo);

        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.imports_of(&solo).is_empty());
        assert!(graph.importers_of(&solo).is_empty());
    }
}
