//! Implements [LSPARCH-FEATURES-REFS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-REFS
//!
//! String/comment skip-mask for raw identifier sweeps.
//!
//! Reference finding, rename, document highlight, and code-lens counts locate
//! candidate identifiers by scanning source text. Occurrences inside string
//! literals, docstrings, and `#` comments are data — not references — and must
//! be skipped, so those sweeps consult a [`SourceMask`] built once per sweep
//! from ruff's lexer and AST (never by quote counting).
//!
//! Deliberately NOT masked, because they are real references:
//! - f-string interpolation fields (`f"hi {name}"` — `name` is code, and the
//!   lexer emits it as a normal `Name` token outside any string token);
//! - string annotations (PEP 563: `def f(x: "MyClass")` — exempted via the
//!   AST's annotation spans).

use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::{walk_stmt, Visitor};
use ruff_python_ast::{ModModule, Stmt};
use ruff_text_size::Ranged;

/// Byte ranges of one source file where identifier matches must be ignored.
///
/// Built by [`SourceMask::build`]; queried per candidate match with
/// [`SourceMask::is_masked`].
#[derive(Debug)]
pub struct SourceMask<'src> {
    /// The source the mask was built from (needed by the parse-failure
    /// fallback, which re-inspects the offset's line).
    source: &'src str,
    /// Sorted, disjoint half-open `[start, end)` masked byte ranges.
    ranges: Vec<(u32, u32)>,
    /// Source failed to parse: fall back to the line-comment heuristic so
    /// behaviour on broken files degrades to comment-only filtering.
    parse_failed: bool,
}

impl<'src> SourceMask<'src> {
    /// Build the mask by parsing `source` with ruff.
    #[must_use]
    pub fn build(source: &'src str) -> Self {
        ruff_python_parser::parse_module(source).map_or(
            Self {
                source,
                ranges: Vec::new(),
                parse_failed: true,
            },
            |parsed| {
                let annotations = annotation_spans(parsed.syntax());
                let ranges = masked_token_ranges(parsed.tokens(), &annotations);
                Self {
                    source,
                    ranges,
                    parse_failed: false,
                }
            },
        )
    }

    /// Whether the byte at `offset` lies inside masked (string/comment) text.
    #[must_use]
    pub fn is_masked(&self, offset: usize) -> bool {
        if self.parse_failed {
            return is_in_line_comment(self.source, offset);
        }
        let Ok(offset) = u32::try_from(offset) else {
            return false;
        };
        self.ranges
            .binary_search_by(|&(start, end)| {
                if offset < start {
                    std::cmp::Ordering::Greater
                } else if offset >= end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }
}

/// Collect masked byte ranges from the token stream: comments, plain string
/// literals (unless in annotation position), and the literal chunks of
/// f-/t-strings. Interpolation fields lex as ordinary tokens and stay clear.
fn masked_token_ranges(
    tokens: &ruff_python_ast::token::Tokens,
    annotations: &[(u32, u32)],
) -> Vec<(u32, u32)> {
    tokens
        .iter()
        .filter_map(|token| {
            let (kind, range) = token.as_tuple();
            let span = (range.start().to_u32(), range.end().to_u32());
            match kind {
                TokenKind::Comment | TokenKind::FStringMiddle | TokenKind::TStringMiddle => {
                    Some(span)
                }
                TokenKind::String if !within_any(annotations, span) => Some(span),
                _ => None,
            }
        })
        .collect()
}

/// Whether `span` lies entirely inside any of `spans`.
fn within_any(spans: &[(u32, u32)], span: (u32, u32)) -> bool {
    spans
        .iter()
        .any(|&(start, end)| span.0 >= start && span.1 <= end)
}

/// Collect the byte spans of every annotation expression in the module:
/// parameter annotations, return annotations, and `AnnAssign` annotations.
/// String literals inside these spans are PEP 563 forward references — code,
/// not data — and must stay renameable.
fn annotation_spans(module: &ModModule) -> Vec<(u32, u32)> {
    let mut collector = AnnotationSpans(Vec::new());
    for stmt in &module.body {
        collector.visit_stmt(stmt);
    }
    collector.0
}

/// AST visitor accumulating annotation expression spans.
struct AnnotationSpans(Vec<(u32, u32)>);

impl AnnotationSpans {
    fn push_span(&mut self, ranged: &impl Ranged) {
        let range = ranged.range();
        self.0.push((range.start().to_u32(), range.end().to_u32()));
    }
}

impl<'a> Visitor<'a> for AnnotationSpans {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(def) => {
                for param in &def.parameters {
                    if let Some(annotation) = param.annotation() {
                        self.push_span(annotation);
                    }
                }
                if let Some(returns) = &def.returns {
                    self.push_span(returns.as_ref());
                }
            }
            Stmt::AnnAssign(ann) => self.push_span(ann.annotation.as_ref()),
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

/// Parse-failure fallback: whether `offset` sits after a `#` on its line with
/// balanced quotes before the `#`. This is the pre-mask heuristic, retained so
/// unparseable buffers keep today's comment filtering instead of none.
fn is_in_line_comment(source: &str, offset: usize) -> bool {
    let line_start = source
        .get(..offset)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |p| p + 1);
    let Some(line_before) = source.get(line_start..offset) else {
        return false;
    };

    let Some(hash_pos) = line_before.find('#') else {
        return false;
    };
    let Some(before_hash) = line_before.get(..hash_pos) else {
        return false;
    };
    let single_quotes = before_hash.chars().filter(|&c| c == '\'').count();
    let double_quotes = before_hash.chars().filter(|&c| c == '"').count();
    single_quotes % 2 == 0 && double_quotes % 2 == 0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::SourceMask;

    /// Byte offset of the `n`th occurrence of `needle` in `source`.
    fn offset_of(source: &str, needle: &str, nth: usize) -> usize {
        source
            .match_indices(needle)
            .nth(nth)
            .map(|(pos, _)| pos)
            .expect("needle occurrence must exist")
    }

    #[test]
    fn masks_docstrings_and_plain_strings_but_not_code() {
        let source = "def process(total: int) -> int:\n    \"\"\"Compute the total.\"\"\"\n    label: str = \"total is big\"\n    return total\n";
        let mask = SourceMask::build(source);
        // Parameter (occurrence 0) and `return total` (occurrence 3) are code.
        assert!(!mask.is_masked(offset_of(source, "total", 0)));
        assert!(!mask.is_masked(offset_of(source, "total", 3)));
        // Docstring prose (1) and string literal content (2) are data.
        assert!(mask.is_masked(offset_of(source, "total", 1)));
        assert!(mask.is_masked(offset_of(source, "total", 2)));
    }

    #[test]
    fn masks_comments() {
        let source = "total: int = 1  # total of the run\n";
        let mask = SourceMask::build(source);
        assert!(!mask.is_masked(offset_of(source, "total", 0)));
        assert!(mask.is_masked(offset_of(source, "total", 1)));
    }

    #[test]
    fn does_not_mask_fstring_interpolation_fields() {
        let source = "name: str = \"x\"\ngreeting: str = f\"Hello, {name}!\"\n";
        let mask = SourceMask::build(source);
        // The interpolation field is code…
        assert!(!mask.is_masked(offset_of(source, "name", 1)));
        // …but the literal chunk around it is data.
        assert!(mask.is_masked(offset_of(source, "Hello", 0)));
    }

    #[test]
    fn does_not_mask_string_annotations() {
        let source = "def f(x: \"MyClass\", y: list[\"MyClass\"]) -> \"MyClass\":\n    z: \"MyClass\" = x\n    return z\n";
        let mask = SourceMask::build(source);
        for nth in 0..4 {
            assert!(
                !mask.is_masked(offset_of(source, "MyClass", nth)),
                "annotation string occurrence {nth} must stay renameable"
            );
        }
    }

    #[test]
    fn parse_failure_falls_back_to_comment_heuristic() {
        let source = "def broken(:\n    total = 1  # total here\n";
        let mask = SourceMask::build(source);
        // Comment filtering survives the parse failure…
        assert!(mask.is_masked(offset_of(source, "total", 1)));
        // …and code stays unmasked.
        assert!(!mask.is_masked(offset_of(source, "total", 0)));
    }
}
