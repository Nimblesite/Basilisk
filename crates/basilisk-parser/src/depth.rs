//! Implements [CHKARCH-ARCH-PARSEDEPTH]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PARSEDEPTH
//!
//! Pre-parse nesting-depth guard.
//!
//! `ruff_python_parser` is a recursive-descent parser, and Basilisk's resolver
//! and checker walk the resulting AST recursively. Both overflow the thread
//! stack on pathologically nested input — a ~4000-deep bracket expression aborts
//! the whole process with `SIGABRT`, which on the language server manifests as a
//! crash-restart loop. To stay crash-safe — and to match `CPython`, which rejects
//! such input at the *tokenizer* rather than crashing — [`crate::parse_source`]
//! runs this guard before handing the source to the recursive parser.
//!
//! Depth is measured with ruff's *linear* lexer (`lex` + `next_token`): a flat
//! byte scan that never recurses, so measuring the depth can never itself
//! overflow (verified at 20 000 deep). The scan short-circuits at the first
//! violating token, so a pathological file is only lexed up to the offending
//! bracket or indent.

use ruff_python_ast::token::TokenKind;
use ruff_python_parser::lexer::lex;
use ruff_python_parser::Mode;

/// Maximum simultaneously-open brackets (`(`, `[`, `{`), counted cumulatively
/// across all three kinds. Matches `CPython`'s tokenizer `MAXLEVEL` (200): a depth
/// of 200 is accepted, the 201st open bracket is rejected.
const MAX_BRACKET_DEPTH: u32 = 200;

/// Maximum indentation levels. Matches `CPython`'s tokenizer `MAXINDENT` (100):
/// 99 levels are accepted, the 100th is rejected.
const MAX_INDENT_DEPTH: u32 = 99;

/// `CPython`'s `SyntaxError` message for exceeding the bracket nesting limit.
const TOO_MANY_BRACKETS: &str = "too many nested parentheses";

/// `CPython`'s `SyntaxError` message for exceeding the indentation limit.
const TOO_MANY_INDENTS: &str = "too many levels of indentation";

/// Reject source whose bracket or indentation nesting would overflow the
/// recursive parser, returning the `CPython`-equivalent message for the first
/// offending token.
///
/// # Errors
///
/// Returns the message for the limit that was exceeded first in token order, or
/// `Ok(())` when the source is within both limits.
pub(crate) fn check_nesting(source: &str) -> Result<(), &'static str> {
    let mut lexer = lex(source, Mode::Module);
    let mut bracket_depth: u32 = 0;
    let mut indent_depth: u32 = 0;
    loop {
        let kind = lexer.next_token();
        if kind.is_eof() {
            break;
        }
        match kind {
            TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => {
                bracket_depth = bracket_depth.saturating_add(1);
                if bracket_depth > MAX_BRACKET_DEPTH {
                    return Err(TOO_MANY_BRACKETS);
                }
            }
            TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                bracket_depth = bracket_depth.saturating_sub(1);
            }
            TokenKind::Indent => {
                indent_depth = indent_depth.saturating_add(1);
                if indent_depth > MAX_INDENT_DEPTH {
                    return Err(TOO_MANY_INDENTS);
                }
            }
            TokenKind::Dedent => {
                indent_depth = indent_depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}
