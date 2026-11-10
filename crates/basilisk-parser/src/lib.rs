//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Python source parser for Basilisk.
//!
//! This crate is the single entry point into `ruff_python_parser` for the
//! entire Basilisk workspace. All other crates receive a [`ParsedModule`]
//! and work exclusively with `ruff_python_ast` types.

mod depth;
pub mod error;

pub use error::ParseError;
use ruff_python_ast::token::TokenKind;
pub use ruff_python_ast::ModModule;

/// The result of parsing a single Python source file.
#[derive(Debug)]
pub struct ParsedModule {
    /// The parsed module AST node.
    pub ast: ModModule,
    /// The original source text (needed for span-to-line-col mapping).
    pub source: String,
    /// The file path from which the source was loaded (for diagnostics).
    pub path: String,
    /// Exact ranges of Python comment tokens, excluding `#` text in strings.
    pub comment_ranges: Vec<(u32, u32)>,
}

/// Parse a Python source string into a [`ParsedModule`].
///
/// # Errors
///
/// Returns [`ParseError::Syntax`] if the source contains syntax errors or is
/// nested too deeply to parse without overflowing the stack
/// ([CHKARCH-ARCH-PARSEDEPTH]).
pub fn parse_source(source: String, path: String) -> Result<ParsedModule, ParseError> {
    // Implements [CHKARCH-ARCH-PARSEDEPTH]: reject pathologically nested source
    // (measured by the linear lexer) before the recursive parser — and the
    // recursive AST visitors downstream — can overflow the stack.
    if let Err(message) = depth::check_nesting(&source) {
        return Err(ParseError::Syntax {
            path,
            message: message.to_owned(),
        });
    }
    ruff_python_parser::parse_module(&source)
        .map(|parsed| {
            // Most comments are not suppression directives. Avoid walking the
            // full token stream unless a possible directive marker exists;
            // when it does, Ruff still classifies the token so marker text in
            // a string cannot masquerade as a comment.
            let possible_directive = source.contains("# type:") || source.contains("# basilisk:");
            let comment_ranges = if possible_directive {
                parsed
                    .tokens()
                    .iter()
                    .filter(|token| token.kind() == TokenKind::Comment)
                    .map(|token| {
                        let range = token.as_tuple().1;
                        (range.start().to_u32(), range.end().to_u32())
                    })
                    .collect()
            } else {
                Vec::new()
            };
            ParsedModule {
                ast: parsed.into_syntax(),
                source,
                path: path.clone(),
                comment_ranges,
            }
        })
        .map_err(|err| ParseError::Syntax {
            path,
            message: err.to_string(),
        })
}

/// Parse a single type expression — e.g. the contents of a string forward
/// reference inside an annotation. Returns `None` when the text does not
/// parse as an expression (the caller treats it gradually).
#[must_use]
pub fn parse_type_expression(text: &str) -> Option<ruff_python_ast::Expr> {
    ruff_python_parser::parse_expression(text.trim())
        .ok()
        .map(|parsed| *parsed.into_syntax().body)
}

/// The element expressions of a subscript slice: a tuple slice contributes
/// each element (`x[a, b]` → `[a, b]`), any other slice is the single
/// element (`x[a]` → `[a]`).
#[must_use]
pub fn subscript_elements(sub: &ruff_python_ast::ExprSubscript) -> Vec<&ruff_python_ast::Expr> {
    match sub.slice.as_ref() {
        ruff_python_ast::Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        other => vec![other],
    }
}

/// Read a file from disk and parse it.
///
/// # Errors
///
/// Returns [`ParseError::Io`] on read failure or [`ParseError::Syntax`] on
/// parse failure.
pub fn parse_file(path: &str) -> Result<ParsedModule, ParseError> {
    // Routed through `read_tracked` so the result cache can record exactly which
    // files a check read. Inert (identical to `fs::read_to_string`) when no
    // recorder is active. See [CHKCACHE-READSET-FS].
    basilisk_common::fs::read_tracked(std::path::Path::new(path))
        .map_err(|source| ParseError::Io {
            path: path.to_owned(),
            source,
        })
        .and_then(|content| parse_source(content, path.to_owned()))
}
