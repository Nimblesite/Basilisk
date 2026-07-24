//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//!
//! Markdown rendering for one hovered symbol.
//!
//! Every hover answers the same three questions in the same order: *what kind
//! of thing is this*, *what is its exact shape*, and *where did that come
//! from*. [`SymbolCard`] is the one place those are laid out, so a signature
//! from a local `def`, a `.pyi` stub, and the bundled Typeshed snapshot all
//! read identically. Unknown pieces are omitted rather than fabricated.

use std::fmt::Write as _;

/// What a hovered symbol is, as the label that precedes its signature.
///
/// The label is the vocabulary the rest of the LSP already uses for local
/// symbols, so an imported symbol is described exactly like a local one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    Function,
    Method,
    Class,
    Variable,
}

impl SymbolKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Variable => "variable",
        }
    }

    /// The kind of an external symbol, given how it is being accessed.
    pub(crate) const fn of_external(
        kind: &basilisk_resolver::scope::ExternalSymbolKind,
    ) -> Option<Self> {
        use basilisk_resolver::scope::ExternalSymbolKind as External;
        match kind {
            External::Function => Some(Self::Function),
            External::Class => Some(Self::Class),
            External::Variable => Some(Self::Variable),
            // A re-export is a forwarding edge, not a symbol shape of its own.
            External::ReExport => None,
        }
    }
}

/// The declaration's location as a reader wants to see it.
///
/// A snapshot body is addressed by a logical `typeshed:<identity>/…` URI whose
/// identity is a content digest — precise, but 40 characters of noise in a
/// hover bubble, and the provenance annotation beside it already says which
/// snapshot is active. The snapshot-relative path is what identifies the
/// declaration to a reader. A real file keeps its full path, which is
/// actionable: it can be opened.
fn readable_source_path(path: &std::path::Path) -> String {
    let display = path.display().to_string();
    display
        .strip_prefix("typeshed:")
        .and_then(|rest| rest.split_once('/'))
        .map_or(display.clone(), |(_identity, relative)| relative.to_owned())
}

/// Everything hover knows about one symbol.
#[derive(Debug, Default)]
pub(crate) struct SymbolCard {
    /// What the symbol is. `None` when nothing determined it.
    pub(crate) kind: Option<SymbolKind>,
    /// One rendered line per declaration — several for an overload set.
    pub(crate) signatures: Vec<String>,
    /// The symbol's own prose documentation.
    pub(crate) docstring: Option<String>,
    /// The module the declaration was read from, e.g. `logging`.
    pub(crate) module: Option<String>,
    /// Where the type information came from, e.g. `(typeshed)`.
    pub(crate) provenance: Option<String>,
    /// The file the declaration was read from.
    pub(crate) source_path: Option<String>,
}

impl SymbolCard {
    /// A card for a single declaration.
    pub(crate) fn new(kind: Option<SymbolKind>, signature: String) -> Self {
        Self {
            kind,
            signatures: vec![signature],
            ..Self::default()
        }
    }

    /// Attach the origin of the declaration: its module, its provenance
    /// annotation, and the file it was read from.
    pub(crate) fn declared_in(
        mut self,
        module: Option<String>,
        provenance: Option<&basilisk_stubs::TypeProvenance>,
        source_path: Option<&std::path::Path>,
    ) -> Self {
        self.module = module;
        self.provenance = provenance
            .and_then(|p| basilisk_stubs::TypeProvenance::hover_label(*p))
            .map(str::to_owned);
        self.source_path = source_path.map(readable_source_path);
        self
    }

    /// Attach the symbol's documentation.
    pub(crate) fn documented(mut self, docstring: Option<String>) -> Self {
        self.docstring = docstring;
        self
    }

    /// Render the card as the Markdown sections of a hover bubble.
    ///
    /// Returns `None` when there is no signature to show — a card with only an
    /// origin says nothing a user can act on.
    pub(crate) fn render(&self) -> Option<String> {
        if self.signatures.is_empty() {
            return None;
        }
        let prefix = self
            .kind
            .map(|kind| format!("({}) ", kind.label()))
            .unwrap_or_default();
        let body = self
            .signatures
            .iter()
            .map(|signature| format!("{prefix}{signature}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut markdown = format!("```python\n{body}\n```");
        if let Some(docstring) = &self.docstring {
            let _ = write!(markdown, "\n\n{docstring}");
        }
        if let Some(origin) = self.origin_line() {
            let _ = write!(markdown, "\n\n{origin}");
        }
        Some(markdown)
    }

    /// The trailing attribution line: module, provenance, and source file.
    fn origin_line(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(module) = &self.module {
            parts.push(format!("`{module}`"));
        }
        if let Some(provenance) = &self.provenance {
            parts.push(format!("*{provenance}*"));
        }
        if let Some(path) = &self.source_path {
            parts.push(format!("`{path}`"));
        }
        (!parts.is_empty()).then(|| parts.join(" — "))
    }
}
