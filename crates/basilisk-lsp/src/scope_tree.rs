//! Implements [LSPARCH-ARCH-RESOLVED]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-RESOLVED
//!
//! Hierarchical scope tree built from a `ResolvedModule`.
//!
//! Python uses lexical scoping: a name assigned anywhere in a function is local
//! to that function (unless declared `global` or `nonlocal`).  The scope tree
//! maps every name to its defining scope and lets callers find all references
//! to a specific binding — not just text matches of the same identifier.

use basilisk_resolver::{ResolvedModule, Span};
use tower_lsp::lsp_types::Range;

use crate::util::byte_offset_to_position;

// ── Scope node ───────────────────────────────────────────────────────────────

/// The kind of scope a node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// The implicit module-level scope.
    Module,
    /// A `def` / `async def` scope.
    Function,
    /// A `class` scope.  Names defined here are *not* visible to nested
    /// functions (Python's class-scope quirk), but are visible to methods
    /// via `self`.
    Class,
}

/// One node in the scope tree.
#[derive(Debug)]
pub struct ScopeNode {
    /// What kind of scope this is.
    pub kind: ScopeKind,
    /// Byte span of the entire scope body.  For functions and classes this is
    /// `def_span` (which covers the whole definition, not just the keyword).
    /// For the module scope this covers `0..source.len()`.
    pub span: Span,
    /// Names **defined** in this scope (parameters, local assigns, class attrs,
    /// module-level vars, function/class names).
    pub defined_names: Vec<String>,
    /// Indices of child scopes inside `ScopeTree::scopes`.
    pub children: Vec<usize>,
    /// Index of the parent scope (`None` only for the module scope).
    pub parent: Option<usize>,
    /// Optional associated name (function name, class name).
    pub name: Option<String>,
}

// ── Scope tree ───────────────────────────────────────────────────────────────

/// A flat arena of `ScopeNode`s representing the full scope hierarchy of a
/// module.  Index 0 is always the module scope.
#[derive(Debug)]
pub struct ScopeTree {
    /// Arena of scope nodes.  Index 0 is always the module scope.
    pub scopes: Vec<ScopeNode>,
}

impl ScopeTree {
    /// Build the scope tree from a resolved module.
    #[must_use]
    pub fn build(resolved: &ResolvedModule, source_len: usize) -> Self {
        let mut tree = Self { scopes: Vec::new() };

        // Module scope (index 0).
        let module_names = collect_module_names(resolved);

        tree.scopes.push(ScopeNode {
            kind: ScopeKind::Module,
            span: Span::new(0, u32::try_from(source_len).unwrap_or(u32::MAX)),
            defined_names: module_names,
            children: Vec::new(),
            parent: None,
            name: None,
        });

        // Add class scopes.
        for class in &resolved.classes {
            let class_names = collect_class_names(class);
            let class_idx = tree.scopes.len();
            tree.scopes.push(ScopeNode {
                kind: ScopeKind::Class,
                span: class.def_span,
                defined_names: class_names,
                children: Vec::new(),
                parent: Some(0),
                name: Some(class.name.clone()),
            });
            if let Some(module) = tree.scopes.first_mut() {
                module.children.push(class_idx);
            }
        }

        // Add function scopes.
        for func in &resolved.functions {
            let func_names = collect_function_names(func);
            let func_idx = tree.scopes.len();
            tree.scopes.push(ScopeNode {
                kind: ScopeKind::Function,
                span: func.def_span,
                defined_names: func_names,
                children: Vec::new(),
                parent: Some(0),
                name: Some(func.name.clone()),
            });
            if let Some(module) = tree.scopes.first_mut() {
                module.children.push(func_idx);
            }
        }

        // Reparent: assign each non-module scope to its tightest enclosing scope.
        tree.reparent();

        tree
    }

    /// Find the innermost scope containing a byte offset.
    #[must_use]
    pub fn scope_at(&self, offset: usize) -> usize {
        let off = u32::try_from(offset).unwrap_or(u32::MAX);
        let mut current = 0; // module scope
        'outer: while let Some(scope) = self.scopes.get(current) {
            for &child_idx in &scope.children {
                let Some(child) = self.scopes.get(child_idx) else {
                    continue;
                };
                if child.span.contains_offset(off) {
                    current = child_idx;
                    continue 'outer;
                }
            }
            break;
        }
        current
    }

    /// Find the scope that **defines** a name, starting from `scope_idx` and
    /// walking up the parent chain.  Returns `None` if the name is not found
    /// anywhere (e.g. a builtin or an unresolved name).
    #[must_use]
    pub fn defining_scope(&self, name: &str, scope_idx: usize) -> Option<usize> {
        let mut idx = scope_idx;
        loop {
            let scope = self.scopes.get(idx)?;
            if scope.defined_names.iter().any(|n| n == name) {
                return Some(idx);
            }
            idx = scope.parent?;
        }
    }

    /// Collect the byte-offset ranges where `name` is in scope of the binding
    /// defined at `def_scope_idx`, **excluding** child scopes that shadow it.
    ///
    /// Returns spans within `source` where text-matching for the identifier
    /// should be performed.
    #[must_use]
    pub fn visible_ranges(&self, name: &str, def_scope_idx: usize) -> Vec<Span> {
        let Some(scope) = self.scopes.get(def_scope_idx) else {
            return vec![];
        };
        let mut ranges = vec![scope.span];

        // Subtract child scopes that redefine the name (shadow it).
        self.subtract_shadowing_children(name, def_scope_idx, &mut ranges);

        ranges
    }

    /// Recursively remove spans of child scopes that shadow `name`.
    fn subtract_shadowing_children(&self, name: &str, scope_idx: usize, ranges: &mut Vec<Span>) {
        let Some(scope) = self.scopes.get(scope_idx) else {
            return;
        };
        let children: Vec<usize> = scope.children.clone();

        for child_idx in children {
            let Some(child) = self.scopes.get(child_idx) else {
                continue;
            };
            if child.defined_names.iter().any(|n| n == name) {
                // This child shadows the name — remove its span from ranges.
                let mut new_ranges = Vec::new();
                for range in ranges.iter() {
                    subtract_span(*range, child.span, &mut new_ranges);
                }
                *ranges = new_ranges;
            } else {
                // Child doesn't shadow — recurse to check grandchildren.
                self.subtract_shadowing_children(name, child_idx, ranges);
            }
        }
    }

    /// Reparent non-module scopes to their tightest enclosing scope.
    fn reparent(&mut self) {
        let count = self.scopes.len();
        if count <= 1 {
            return;
        }

        // For each scope (skip module at index 0), find the smallest enclosing
        // scope.
        let spans: Vec<Span> = self.scopes.iter().map(|s| s.span).collect();

        for idx in 1..count {
            let span = spans.get(idx).copied().unwrap_or(Span::new(0, 0));
            let mut best_parent = 0usize;
            let mut best_size = u32::MAX;

            for (candidate, cand_span) in spans.iter().enumerate() {
                if candidate == idx {
                    continue;
                }
                let cand_size = cand_span.end.saturating_sub(cand_span.start);
                if cand_span.contains_span(span) && cand_size < best_size {
                    best_parent = candidate;
                    best_size = cand_size;
                }
            }

            if let Some(scope) = self.scopes.get_mut(idx) {
                scope.parent = Some(best_parent);
            }
        }

        // Rebuild children lists from parent pointers.
        for scope in &mut self.scopes {
            scope.children.clear();
        }
        for idx in 1..count {
            let parent = self.scopes.get(idx).and_then(|s| s.parent).unwrap_or(0);
            if let Some(parent_scope) = self.scopes.get_mut(parent) {
                parent_scope.children.push(idx);
            }
        }
    }
}

// ── Name collection helpers ──────────────────────────────────────────────────

/// Collect names defined at module scope.
fn collect_module_names(resolved: &ResolvedModule) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for var in &resolved.module_vars {
        names.push(var.name.clone());
    }
    for func in &resolved.functions {
        if func.class_name.is_none() && !is_nested_function(func, resolved) {
            names.push(func.name.clone());
        }
    }
    for class in &resolved.classes {
        names.push(class.name.clone());
    }
    for imp in &resolved.imports {
        for name in &imp.names {
            names.push(name.clone());
        }
        if imp.names.is_empty() {
            names.push(imp.module.clone());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Collect names defined in a class scope.
fn collect_class_names(class: &basilisk_resolver::ClassInfo) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for attr in &class.attributes {
        names.push(attr.name.clone());
    }
    for method_name in &class.method_names {
        names.push(method_name.clone());
    }
    names.sort();
    names.dedup();
    names
}

/// Collect names defined in a function scope.
fn collect_function_names(func: &basilisk_resolver::FunctionInfo) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for param in &func.parameters {
        names.push(param.name.clone());
    }
    if let Some(ref va) = func.vararg {
        names.push(va.name.clone());
    }
    if let Some(ref kw) = func.kwarg {
        names.push(kw.name.clone());
    }
    for name in &func.all_local_assigns {
        names.push(name.clone());
    }
    for var in &func.local_vars {
        names.push(var.name.clone());
    }
    names.sort();
    names.dedup();
    names
}

// ── Scope-aware reference finding ────────────────────────────────────────────

/// Find all references to the binding at `byte_offset` in `source`, respecting
/// Python's lexical scoping rules.
///
/// Returns LSP `Range`s for each reference (including the definition itself).
#[must_use]
pub fn find_scoped_references(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Vec<Range> {
    let Some(name) = crate::util::identifier_at_offset(source, byte_offset) else {
        return vec![];
    };

    let tree = ScopeTree::build(resolved, source.len());
    let scope_idx = tree.scope_at(byte_offset);

    // Find which scope defines this name.
    let Some(def_scope) = tree.defining_scope(&name, scope_idx) else {
        // Name not found in any scope — fall back to module-wide text match.
        return crate::references::find_identifier_occurrences(source, &name);
    };

    // Get the visible ranges (defining scope minus shadowing children).
    let visible = tree.visible_ranges(&name, def_scope);

    // Text-match within each visible range.
    let mut results = Vec::new();
    for span in &visible {
        let start = span.start_usize();
        let end = span.end_usize().min(source.len());
        let Some(slice) = source.get(start..end) else {
            continue;
        };

        find_identifier_in_slice(slice, &name, start, source, &mut results);
    }

    results
}

/// Text-match `name` as a whole-word identifier within `slice`, converting
/// matches to LSP ranges using absolute offsets into `full_source`.
fn find_identifier_in_slice(
    slice: &str,
    name: &str,
    slice_offset: usize,
    full_source: &str,
    results: &mut Vec<Range>,
) {
    let bytes = slice.as_bytes();
    let name_len = name.len();
    let mut pos = 0;

    while let Some(rel) = slice.get(pos..).and_then(|s| s.find(name)) {
        let match_start = pos + rel;
        let match_end = match_start + name_len;

        let at_word_start =
            match_start == 0 || bytes.get(match_start - 1).is_none_or(|&b| !is_ident(b));
        let at_word_end = bytes.get(match_end).is_none_or(|&b| !is_ident(b));

        if at_word_start
            && at_word_end
            && !crate::references::is_in_string_or_comment(full_source, slice_offset + match_start)
        {
            let lsp_start = byte_offset_to_position(full_source, slice_offset + match_start);
            let lsp_end = byte_offset_to_position(full_source, slice_offset + match_end);
            results.push(Range {
                start: lsp_start,
                end: lsp_end,
            });
        }

        pos = match_start + name_len.max(1);
    }
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── Rename validation ────────────────────────────────────────────────────────

/// Reasons a rename can be rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameRejection {
    /// The new name is not a valid Python identifier.
    InvalidIdentifier,
    /// The new name would shadow an existing binding in the same scope.
    WouldShadow {
        /// Human-readable description of the scope that already defines the name.
        existing_scope: String,
    },
    /// The new name conflicts with a Python builtin.
    ConflictsWithBuiltin,
}

/// Python builtins that a rename should warn about.
const PYTHON_BUILTINS: &[&str] = &[
    "abs",
    "all",
    "any",
    "ascii",
    "bin",
    "bool",
    "breakpoint",
    "bytearray",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "copyright",
    "credits",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "exit",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "license",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "quit",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
    "None",
    "True",
    "False",
    "NotImplemented",
    "Ellipsis",
    "Any",
    "Optional",
    "Union",
    "List",
    "Dict",
    "Tuple",
    "Set",
    "Type",
];

/// Python keywords that are never valid identifiers.
const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// Validate a proposed rename.  Returns `None` if the rename is acceptable,
/// or `Some(rejection)` explaining why it is not.
#[must_use]
pub fn validate_rename(
    new_name: &str,
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<RenameRejection> {
    if !is_valid_python_identifier(new_name) {
        return Some(RenameRejection::InvalidIdentifier);
    }

    if PYTHON_KEYWORDS.contains(&new_name) {
        return Some(RenameRejection::InvalidIdentifier);
    }

    if PYTHON_BUILTINS.contains(&new_name) {
        return Some(RenameRejection::ConflictsWithBuiltin);
    }

    // Check for shadowing in the current scope.
    let tree = ScopeTree::build(resolved, source.len());
    let scope_idx = tree.scope_at(byte_offset);

    let old_name = crate::util::identifier_at_offset(source, byte_offset)?;
    let def_scope = tree.defining_scope(&old_name, scope_idx)?;

    let scope = tree.scopes.get(def_scope)?;
    if scope.defined_names.iter().any(|n| n == new_name) {
        let scope_desc = match scope.kind {
            ScopeKind::Module => "module".to_owned(),
            ScopeKind::Function => {
                format!("function `{}`", scope.name.as_deref().unwrap_or("?"))
            }
            ScopeKind::Class => {
                format!("class `{}`", scope.name.as_deref().unwrap_or("?"))
            }
        };
        return Some(RenameRejection::WouldShadow {
            existing_scope: scope_desc,
        });
    }

    None
}

/// Check if a string is a valid Python identifier.
fn is_valid_python_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

// ── Span helpers ─────────────────────────────────────────────────────────────

/// Check if a function is nested inside another function (not a direct method).
fn is_nested_function(func: &basilisk_resolver::FunctionInfo, resolved: &ResolvedModule) -> bool {
    if func.class_name.is_some() {
        return false;
    }
    for other in &resolved.functions {
        if std::ptr::eq(func, other) {
            continue;
        }
        if other.def_span.contains_span(func.def_span) {
            return true;
        }
    }
    false
}

/// Subtract `hole` from `outer`, pushing the remaining pieces into `out`.
fn subtract_span(outer: Span, hole: Span, out: &mut Vec<Span>) {
    if hole.start >= outer.end || hole.end <= outer.start {
        out.push(outer);
        return;
    }
    if hole.start > outer.start {
        out.push(Span::new(outer.start, hole.start));
    }
    if hole.end < outer.end {
        out.push(Span::new(hole.end, outer.end));
    }
}
