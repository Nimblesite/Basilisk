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
//! Three limits are enforced:
//!
//! - **Brackets** and **indentation**, matching `CPython`'s own tokenizer caps.
//! - **Operator chains**: a flat token stream can still build an arbitrarily
//!   deep AST — `total = 1 + 1 + …` parses without any bracket nesting but
//!   yields one left-nested `BinOp` per term, and the recursive visitors
//!   overflow even the 64 MiB analysis stacks of [LSPARCH-ARCH-STACK] at
//!   ~150,000 levels (GitHub #278). The guard counts depth-building tokens
//!   (binary/unary operators, `.`, ternary `if`/`else`, `lambda`) per
//!   expression context and rejects runs past [`MAX_EXPR_OPERATORS`].
//!
//! Depth is measured with ruff's *linear* lexer (`lex` + `next_token`): a flat
//! byte scan that never recurses, so measuring the depth can never itself
//! overflow (verified at 20 000 deep). The scan short-circuits at the first
//! violating token, so a pathological file is only lexed up to the offending
//! bracket, indent, or operator.

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

/// Maximum depth-building operator tokens in one uninterrupted expression
/// context (per bracket level, reset at `,` `;` `=` and logical newlines).
///
/// An operator chain of length N nests the AST at most N deep, so this bounds
/// visitor recursion. Measured on the worst construct (a `1 + 1 + …` `BinOp`
/// chain): 100,000 levels analyse cleanly on the 64 MiB analysis stacks of
/// [LSPARCH-ARCH-STACK] even in debug builds (~150,000 is the crash floor),
/// and attribute/ternary/unary chains survive past 75,000 — 50,000 keeps a
/// >2× margin while staying far beyond any legitimate generated file.
const MAX_EXPR_OPERATORS: u32 = 50_000;

/// `CPython`'s `SyntaxError` message for exceeding the bracket nesting limit.
const TOO_MANY_BRACKETS: &str = "too many nested parentheses";

/// `CPython`'s `SyntaxError` message for exceeding the indentation limit.
const TOO_MANY_INDENTS: &str = "too many levels of indentation";

/// Message for exceeding [`MAX_EXPR_OPERATORS`] — the construct `CPython`'s
/// tokenizer caps don't reach, so the wording follows their style.
const TOO_DEEP_EXPRESSION: &str = "expression too deeply nested";

/// Whether `kind` can add a level of AST nesting without opening a bracket.
///
/// Flat-by-construction operators are deliberately absent: `and`/`or` build
/// one `BoolOp` with a value *list*, chained comparisons one `Compare` with a
/// comparator *list*, and `,` a single tuple/list/call node — none of them
/// deepen the tree, and giant flat literals are legitimate generated code.
fn builds_ast_depth(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::DoubleStar
            | TokenKind::Slash
            | TokenKind::DoubleSlash
            | TokenKind::Percent
            | TokenKind::At
            | TokenKind::Amper
            | TokenKind::Vbar
            | TokenKind::CircumFlex
            | TokenKind::Tilde
            | TokenKind::LeftShift
            | TokenKind::RightShift
            | TokenKind::Dot
            | TokenKind::Not
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::Lambda
    )
}

/// Whether `kind` terminates the current operator chain: a new expression
/// starts after it, so accumulated depth cannot carry across.
fn breaks_operator_chain(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Newline | TokenKind::Comma | TokenKind::Semi | TokenKind::Equal
    )
}

/// Operator-chain lengths per open-bracket level. Each nested bracket starts
/// a fresh chain; closing it resumes the enclosing one (so `1 + f(x) + 1`
/// keeps counting the outer chain across the call).
struct OperatorChains {
    current: u32,
    outer: Vec<u32>,
}

impl OperatorChains {
    fn new() -> Self {
        Self {
            current: 0,
            outer: Vec::new(),
        }
    }

    fn enter_bracket(&mut self) {
        self.outer.push(self.current);
        self.current = 0;
    }

    fn exit_bracket(&mut self) {
        // An unbalanced closer (a syntax error the parser reports later)
        // resumes at zero rather than corrupting the module-level chain.
        self.current = self.outer.pop().unwrap_or(0);
    }

    fn break_chain(&mut self) {
        self.current = 0;
    }

    /// Count one depth-building operator in the current context.
    ///
    /// # Errors
    ///
    /// Returns [`TOO_DEEP_EXPRESSION`] when the chain exceeds
    /// [`MAX_EXPR_OPERATORS`].
    fn count_operator(&mut self) -> Result<(), &'static str> {
        self.current = self.current.saturating_add(1);
        if self.current > MAX_EXPR_OPERATORS {
            return Err(TOO_DEEP_EXPRESSION);
        }
        Ok(())
    }
}

/// Reject source whose bracket, indentation, or operator-chain nesting would
/// overflow the recursive parser or the recursive AST visitors downstream,
/// returning the `CPython`-equivalent message for the first offending token.
///
/// # Errors
///
/// Returns the message for the limit that was exceeded first in token order, or
/// `Ok(())` when the source is within all limits.
pub(crate) fn check_nesting(source: &str) -> Result<(), &'static str> {
    let mut lexer = lex(source, Mode::Module);
    let mut bracket_depth: u32 = 0;
    let mut indent_depth: u32 = 0;
    let mut chains = OperatorChains::new();
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
                chains.enter_bracket();
            }
            TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                bracket_depth = bracket_depth.saturating_sub(1);
                chains.exit_bracket();
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
            kind if builds_ast_depth(kind) => chains.count_operator()?,
            kind if breaks_operator_chain(kind) => chains.break_chain(),
            _ => {}
        }
    }
    Ok(())
}
