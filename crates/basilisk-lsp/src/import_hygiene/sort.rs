//! Implements [LSPFMT-IMPORTS] — organize imports with isort semantics,
//! natively on the Ruff AST.
//!
//! Behavior parity with Ruff's `I001` fixer (verified against `ruff` 0.15.17
//! on representative fixtures):
//!
//! - Sections in order: `__future__`, stdlib, third-party, relative — one
//!   blank line between sections, blank lines inside the block collapsed.
//! - Within a section: all `import x` statements first (sorted), then all
//!   `from x import ...` statements (sorted); module names compare
//!   case-insensitively; `import a` sorts before `import a as z`.
//! - `import a, b` is split into one statement per module.
//! - Duplicate imports are dropped; same-module `from` imports merge, with
//!   members ordered constants → classes → others (isort `order-by-type`).
//! - Trailing same-line comments move with their statement.
//! - `from` imports longer than the line width wrap in parentheses, one
//!   member per line with a trailing comma.
//!
//! The organizer refuses (returns `None`) rather than guess when the block
//! contains standalone comment lines, comments inside parenthesized imports,
//! or comment placements a merge would have to discard — never destroy a
//! comment.

use ruff_python_ast::token::TokenKind;
use ruff_python_ast::Stmt;
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;

/// Ruff's default line width, used when the project sets none.
const DEFAULT_LINE_WIDTH: usize = 88;

/// Canonical Python minor version for stdlib-module classification (3.12).
const PYTHON_MINOR: u8 = 12;

/// Import sections in output order (isort defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Future,
    Stdlib,
    ThirdParty,
    LocalRelative,
}

/// `import module [as alias]`.
struct StraightImport {
    module: String,
    asname: Option<String>,
    comment: Option<String>,
}

/// One name inside a `from` import.
#[derive(PartialEq, Eq)]
struct Member {
    name: String,
    asname: Option<String>,
}

/// `from [.]*module import a, b as c`.
struct FromImport {
    level: u32,
    module: Option<String>,
    members: Vec<Member>,
    comment: Option<String>,
}

impl FromImport {
    /// Sort/merge key: dots for relative levels plus the module path.
    fn module_key(&self) -> String {
        let dots = ".".repeat(usize::try_from(self.level).unwrap_or(0));
        format!("{dots}{}", self.module.as_deref().unwrap_or(""))
    }
}

/// Classify an import into its isort section. Unknown top-level modules are
/// third-party, matching Ruff's default without project configuration.
fn classify(level: u32, module: Option<&str>) -> Section {
    if level > 0 {
        return Section::LocalRelative;
    }
    match module {
        Some("__future__") => Section::Future,
        Some(path) => {
            let base = path.split('.').next().unwrap_or(path);
            if ruff_python_stdlib::sys::is_known_standard_library(PYTHON_MINOR, base) {
                Section::Stdlib
            } else {
                Section::ThirdParty
            }
        }
        None => Section::LocalRelative,
    }
}

/// isort `order-by-type` member rank: CONSTANTS, then Classes, then the rest.
fn member_rank(name: &str) -> u8 {
    let has_alpha = name.chars().any(|c| c.is_ascii_alphabetic());
    if has_alpha && !name.chars().any(|c| c.is_ascii_lowercase()) {
        0
    } else if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        1
    } else {
        2
    }
}

/// The leading run of import statements (a docstring may precede it).
fn leading_import_block(body: &[Stmt]) -> Vec<&Stmt> {
    let mut block = Vec::new();
    for (index, stmt) in body.iter().enumerate() {
        match stmt {
            Stmt::Import(_) | Stmt::ImportFrom(_) => block.push(stmt),
            Stmt::Expr(expr) if index == 0 && expr.value.is_string_literal_expr() => {}
            _ => break,
        }
    }
    block
}

/// Sort the leading import block of `source` with isort semantics.
///
/// Returns the full rewritten source, or `None` when there are no imports,
/// the block is already organized, the source does not parse, or comments
/// are placed such that reordering would have to destroy them.
#[must_use]
pub fn organize_source(source: &str, line_length: Option<u16>) -> Option<String> {
    let parsed = parse_module(source).ok()?;
    let block = leading_import_block(&parsed.syntax().body);
    let block_start = usize::from(block.first()?.range().start());
    let mut block_end = usize::from(block.last()?.range().end());

    let comments: Vec<(usize, usize)> = parsed
        .tokens()
        .iter()
        .filter(|token| token.kind() == TokenKind::Comment)
        .map(|token| {
            (
                usize::from(token.range().start()),
                usize::from(token.range().end()),
            )
        })
        .collect();

    // A trailing comment on the last import's line belongs to the block.
    for &(comment_start, comment_end) in &comments {
        if comment_start >= block_end
            && !source.get(block_end..comment_start)?.contains('\n')
            && source.get(block_end..comment_start)?.trim().is_empty()
        {
            block_end = comment_end;
        }
    }

    // Attach each in-block comment to the statement it trails; refuse when a
    // comment stands alone or sits inside a multi-line statement.
    let mut stmt_comments: Vec<Option<String>> = block.iter().map(|_| None).collect();
    for &(comment_start, comment_end) in &comments {
        if comment_start < block_start || comment_start >= block_end {
            continue;
        }
        let owner = block.iter().position(|stmt| {
            let stmt_end = usize::from(stmt.range().end());
            comment_start >= stmt_end
                && source
                    .get(stmt_end..comment_start)
                    .is_some_and(|between| !between.contains('\n') && between.trim().is_empty())
        })?;
        let text = source.get(comment_start..comment_end)?.to_owned();
        *stmt_comments.get_mut(owner)? = Some(text);
    }

    let (straights, froms) = collect_items(&block, &stmt_comments)?;
    let new_block = render(straights, froms, line_length);

    if source.get(block_start..block_end)? == new_block {
        return None;
    }
    Some(format!(
        "{}{new_block}{}",
        source.get(..block_start)?,
        source.get(block_end..)?
    ))
}

/// Convert block statements into straight/from items, splitting multi-alias
/// imports, deduplicating, and merging same-module `from` imports.
fn collect_items(
    block: &[&Stmt],
    stmt_comments: &[Option<String>],
) -> Option<(Vec<StraightImport>, Vec<FromImport>)> {
    let mut straights: Vec<StraightImport> = Vec::new();
    let mut froms: Vec<FromImport> = Vec::new();

    for (stmt, comment) in block.iter().zip(stmt_comments) {
        match stmt {
            Stmt::Import(import) => {
                // A comment on `import a, b` has no single owner after the
                // split — refuse rather than misattribute it.
                if import.names.len() > 1 && comment.is_some() {
                    return None;
                }
                for alias in &import.names {
                    let item = StraightImport {
                        module: alias.name.to_string(),
                        asname: alias.asname.as_ref().map(ToString::to_string),
                        comment: comment.clone(),
                    };
                    match straights
                        .iter_mut()
                        .find(|s| s.module == item.module && s.asname == item.asname)
                    {
                        Some(existing) => merge_comment(&mut existing.comment, item.comment)?,
                        None => straights.push(item),
                    }
                }
            }
            Stmt::ImportFrom(import) => {
                let members = import
                    .names
                    .iter()
                    .map(|alias| Member {
                        name: alias.name.to_string(),
                        asname: alias.asname.as_ref().map(ToString::to_string),
                    })
                    .collect();
                let item = FromImport {
                    level: import.level,
                    module: import.module.as_ref().map(ToString::to_string),
                    members,
                    comment: comment.clone(),
                };
                match froms
                    .iter_mut()
                    .find(|f| f.level == item.level && f.module == item.module)
                {
                    Some(existing) => {
                        merge_comment(&mut existing.comment, item.comment)?;
                        for member in item.members {
                            if !existing.members.contains(&member) {
                                existing.members.push(member);
                            }
                        }
                    }
                    None => froms.push(item),
                }
            }
            _ => return None,
        }
    }
    Some((straights, froms))
}

/// Keep the single comment across a merge; refuse when both sides carry one.
fn merge_comment(existing: &mut Option<String>, incoming: Option<String>) -> Option<()> {
    match (existing.as_ref(), incoming) {
        (Some(_), Some(_)) => None,
        (None, Some(comment)) => {
            *existing = Some(comment);
            Some(())
        }
        (_, None) => Some(()),
    }
}

/// Render the organized block: sections in order, straight imports before
/// `from` imports within each section.
fn render(
    mut straights: Vec<StraightImport>,
    mut froms: Vec<FromImport>,
    line_length: Option<u16>,
) -> String {
    let width = line_length.map_or(DEFAULT_LINE_WIDTH, usize::from);

    straights.sort_by(|a, b| {
        (
            a.module.to_lowercase(),
            &a.module,
            a.asname.is_some(),
            &a.asname,
        )
            .cmp(&(
                b.module.to_lowercase(),
                &b.module,
                b.asname.is_some(),
                &b.asname,
            ))
    });
    froms.sort_by_key(|f| (f.module_key().to_lowercase(), f.module_key()));
    for from in &mut froms {
        from.members.sort_by(|a, b| {
            (
                member_rank(&a.name),
                a.name.to_lowercase(),
                &a.name,
                &a.asname,
            )
                .cmp(&(
                    member_rank(&b.name),
                    b.name.to_lowercase(),
                    &b.name,
                    &b.asname,
                ))
        });
    }

    let mut sections: Vec<(Section, Vec<String>)> = Vec::new();
    let mut push = |section: Section, line: String| match sections.last_mut() {
        Some((last, lines)) if *last == section => lines.push(line),
        _ => sections.push((section, vec![line])),
    };

    for section in [
        Section::Future,
        Section::Stdlib,
        Section::ThirdParty,
        Section::LocalRelative,
    ] {
        for straight in straights
            .iter()
            .filter(|s| classify(0, Some(&s.module)) == section)
        {
            push(section, render_straight(straight));
        }
        for from in froms
            .iter()
            .filter(|f| classify(f.level, f.module.as_deref()) == section)
        {
            push(section, render_from(from, width));
        }
    }

    sections
        .iter()
        .map(|(_, lines)| lines.join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_straight(import: &StraightImport) -> String {
    let mut line = match &import.asname {
        Some(asname) => format!("import {} as {asname}", import.module),
        None => format!("import {}", import.module),
    };
    append_comment(&mut line, import.comment.as_deref());
    line
}

fn render_from(import: &FromImport, width: usize) -> String {
    let rendered_members: Vec<String> = import
        .members
        .iter()
        .map(|member| match &member.asname {
            Some(asname) => format!("{} as {asname}", member.name),
            None => member.name.clone(),
        })
        .collect();

    let mut single = format!(
        "from {} import {}",
        import.module_key(),
        rendered_members.join(", ")
    );
    append_comment(&mut single, import.comment.as_deref());
    if single.chars().count() <= width || rendered_members.len() == 1 {
        return single;
    }

    // Wrap like Ruff: parenthesized, one member per line, trailing comma.
    let mut wrapped = format!("from {} import (", import.module_key());
    append_comment(&mut wrapped, import.comment.as_deref());
    for member in &rendered_members {
        wrapped.push_str("\n    ");
        wrapped.push_str(member);
        wrapped.push(',');
    }
    wrapped.push_str("\n)");
    wrapped
}

fn append_comment(line: &mut String, comment: Option<&str>) {
    if let Some(comment) = comment {
        line.push_str("  ");
        line.push_str(comment);
    }
}
