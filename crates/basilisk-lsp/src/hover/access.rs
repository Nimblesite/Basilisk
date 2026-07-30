//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//!
//! How the identifier under the cursor is reached, and what type the value
//! before the dot has.
//!
//! `imported_symbols` is a flat, module-agnostic map: a plain `import os`
//! publishes every member of `os` under its own bare name, so typeshed's
//! `error = OSError` occupies the key `"error"`. Answering `logger.error(...)`
//! from that key is a category error — the map may only be consulted for a
//! name the module actually *binds*. This module draws that line.

use basilisk_resolver::{ImportInfo, ImportKind, ResolvedModule, Span};

use crate::util::{annotation_text, find_definition_by_name, SymbolHit};

/// How the identifier under the cursor is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Access {
    /// A free name — `getLogger(...)`, `MyClass`, `count`. Module-level
    /// bindings and imported names both apply.
    Free,
    /// A member access — `logger.error`, `os.getcwd`, `" ".join`.
    Member {
        /// The receiver when it is a simple name; `None` when the receiver is
        /// a literal or a compound expression (`" ".join`, `f().x`).
        receiver: Option<String>,
    },
}

impl Access {
    /// The receiver name, when this is a member access on a named receiver.
    pub(crate) fn receiver(&self) -> Option<&str> {
        match self {
            Self::Free => None,
            Self::Member { receiver } => receiver.as_deref(),
        }
    }

    /// Whether this access reaches the identifier through a `.`.
    pub(crate) const fn is_member(&self) -> bool {
        matches!(self, Self::Member { .. })
    }
}

/// Classify the access at `byte_offset`.
///
/// The identifier is a member access exactly when the first non-whitespace
/// character before it is a `.` — independent of whether the receiver is a
/// name, so `" ".join` is classified as a member access too.
pub(crate) fn access_at(source: &str, byte_offset: usize) -> Access {
    let before = source
        .get(..byte_offset.min(source.len()))
        .unwrap_or(source);
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    let Some(before_dot) = stripped.trim_end().strip_suffix('.') else {
        return Access::Free;
    };
    let receiver: String = before_dot
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Access::Member {
        receiver: (!receiver.is_empty()).then_some(receiver),
    }
}

/// The external symbol a module-qualified access names, e.g. `os.getcwd`.
///
/// The flat `imported_symbols` entry is trusted only when it came from a file
/// that one of `receiver`'s plain imports actually resolved to. Without that
/// check `logging`'s and `os`'s same-named exports are indistinguishable, and
/// whichever import was processed last answers for both.
pub(crate) fn module_member<'a>(
    resolved: &'a ResolvedModule,
    receiver: &str,
    member: &str,
) -> Option<&'a basilisk_resolver::scope::ExternalSymbol> {
    let symbol = resolved.imported_symbols.get(member)?;
    module_imports_for(resolved, receiver)
        .any(|import| import.resolved_path.as_deref() == Some(symbol.source_path.as_path()))
        .then_some(symbol)
}

/// The plain imports that bind `receiver` as a module object.
///
/// `import os` and `import os.path as p` both bind a module; `import os.path`
/// binds only the leading `os` segment.
fn module_imports_for<'a>(
    resolved: &'a ResolvedModule,
    receiver: &'a str,
) -> impl Iterator<Item = &'a ImportInfo> {
    resolved.imports.iter().filter(move |import| {
        import.kind == ImportKind::Plain
            && (import.names.iter().any(|name| name == receiver)
                || import.module == receiver
                || (import.names.is_empty() && import.module.split('.').next() == Some(receiver)))
    })
}

/// Find the innermost class that encloses `offset`.
///
/// A `self.` expression always sits inside a method body, which always
/// follows its own `def` — so the enclosing class is the one owning the
/// NEAREST PRECEDING method start, not the first method in the file
/// (regression: members of the file's first class leaked into every later
/// class).
pub(crate) fn enclosing_class(
    resolved: &ResolvedModule,
    offset: usize,
) -> Option<&basilisk_resolver::scope::ClassInfo> {
    let func = resolved
        .functions
        .iter()
        .filter(|f| f.class_name.is_some() && f.def_span.start_usize() <= offset)
        .max_by_key(|f| f.def_span.start_usize())?;
    let class_name = func.class_name.as_ref()?;
    resolved.classes.iter().find(|c| &c.name == class_name)
}

/// The name of the type a value receiver has.
///
/// Resolved from the receiver's own declaration only — its annotation, the
/// literal it was assigned, or the return type of the call that produced it
/// (`logger = logging.getLogger(...)` → `Logger`). Nothing is guessed: an
/// unresolvable receiver yields `None` so hover stays silent rather than
/// answering for the wrong symbol ([LSPARCH-FEATURES-HOVER]).
///
/// The `bool` reports a `LiteralString`-compatible receiver, which selects the
/// `LiteralString` overloads of built-in `str` methods.
pub(crate) fn receiver_type_name(
    resolved: &ResolvedModule,
    source: &str,
    receiver: &str,
) -> Option<(String, bool)> {
    let (annotation_span, rhs_kind, rhs_span) = match find_definition_by_name(resolved, receiver)? {
        SymbolHit::Variable(var) => (var.annotation_span, Some(&var.rhs_kind), var.rhs_span),
        SymbolHit::Parameter { param, .. } => (param.annotation_span, None, None),
        _ => return None,
    };
    if let Some(annotation) = annotation_text(annotation_span, source) {
        let literal = annotation == "LiteralString" || annotation == "typing.LiteralString";
        return Some((
            if literal {
                "str".to_owned()
            } else {
                annotation
            },
            literal,
        ));
    }
    let inferred = crate::util::rhs_or_expr_type_display(rhs_kind?, rhs_span, source);
    if inferred.is_empty() {
        return call_return_type(resolved, rhs_span).map(|name| (name, false));
    }
    let literal =
        inferred == "str" && matches!(rhs_kind, Some(basilisk_resolver::RhsKind::StrLiteral));
    Some((inferred, literal))
}

/// The declared return type of the call that produced a variable.
///
/// The resolver already recorded the call site — its callee and receiver come
/// from the AST, never from re-scanning source text.
fn call_return_type(resolved: &ResolvedModule, rhs_span: Option<Span>) -> Option<String> {
    let span = rhs_span?;
    let call = resolved.calls.iter().find(|call| call.span == span)?;
    let declared = match &call.receiver {
        None => free_callee_return_type(resolved, &call.callee),
        Some(basilisk_resolver::scope::CallReceiver::Name(module)) => {
            module_member(resolved, module, &call.callee)?
                .type_annotation
                .clone()
        }
        Some(_) => None,
    }?;
    bare_type_name(&declared)
}

/// The return type declared for a free callee: a local class constructor, a
/// local function's return annotation, or an imported declaration.
fn free_callee_return_type(resolved: &ResolvedModule, callee: &str) -> Option<String> {
    if resolved.classes.iter().any(|class| class.name == callee) {
        return Some(callee.to_owned());
    }
    if let Some(func) = resolved
        .functions
        .iter()
        .find(|func| func.name == callee && func.class_name.is_none())
    {
        if let Some(annotation) = annotation_text(func.return_annotation_span, &resolved.source) {
            return Some(annotation);
        }
    }
    let symbol = resolved.imported_symbols.get(callee)?;
    match symbol.kind {
        basilisk_resolver::scope::ExternalSymbolKind::Class => Some(symbol.name.clone()),
        _ => symbol.type_annotation.clone(),
    }
}

/// Reduce a declared type to the bare class name hover can look up.
///
/// `logging.Logger` → `Logger`, `"Logger"` → `Logger`, `list[int]` → `list`.
/// A union or any other compound form has no single declaring class, so it
/// yields `None` rather than an arbitrary member of it.
fn bare_type_name(declared: &str) -> Option<String> {
    let trimmed = declared.trim().trim_matches(['\'', '"']).trim();
    let head = trimmed.split('[').next().unwrap_or(trimmed).trim();
    let bare = head.rsplit('.').next().unwrap_or(head);
    let plain = !bare.is_empty()
        && bare.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !bare.starts_with(|c: char| c.is_ascii_digit());
    plain.then(|| bare.to_owned())
}
