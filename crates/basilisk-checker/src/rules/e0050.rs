//! BSK-E0050: Invalid `NewType(...)` call.
//!
//! PEP 484 places restrictions on `NewType`:
//!
//! - The string name must match the variable it is assigned to
//! - The base type must be a proper concrete class
//! - `NewType` accepts exactly two arguments
//!
//! ```python
//! from typing import NewType
//! GoodName = NewType("BadName", int)  # E: name mismatch
//! BadNewType6 = NewType("BadNewType6", int, int)  # E: too many arguments
//! BadNewType7 = NewType("BadNewType7", Any)  # E: cannot be Any
//! ```

use basilisk_resolver::{NewTypeCallInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0050",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0050",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "`NewType` requires exactly two arguments: a string name and a concrete base class"
                .to_owned(),
        ),
        note: Some(
            "PEP 484: `NewType` accepts only proper concrete classes as the base type"
                .to_owned(),
        ),
    }
}

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

/// Known abstract protocol base classes that cannot be used as a `NewType` base.
const KNOWN_PROTOCOLS: &[&str] = &[
    "Hashable",
    "Iterable",
    "Iterator",
    "Generator",
    "Sized",
    "Container",
    "Collection",
    "Callable",
    "Sequence",
    "MutableSequence",
    "Mapping",
    "MutableMapping",
    "Set",
    "MutableSet",
    "Awaitable",
    "Coroutine",
    "AsyncIterable",
    "AsyncIterator",
    "AsyncGenerator",
    "Buffer",
    "SupportsInt",
    "SupportsFloat",
    "SupportsComplex",
    "SupportsBytes",
    "SupportsAbs",
    "SupportsRound",
];

/// Returns an error reason if the base type text is invalid for `NewType`.
fn is_invalid_base(base: &str, typeddict_names: &[&str]) -> Option<&'static str> {
    let base = base.trim();

    // `Any`
    if base == "Any" {
        return Some("cannot use `Any` as a `NewType` base");
    }

    // `Literal[...]`
    if base.starts_with("Literal[") || base.starts_with("Literal [") {
        return Some("cannot use `Literal` as a `NewType` base");
    }

    // Union type with `|` operator at depth 0
    if has_top_level_union(base) {
        return Some("cannot use a union type as a `NewType` base");
    }

    // TypeVar-parameterized subscript: `list[T]`
    if is_typevar_parameterized_subscript(base) {
        return Some("cannot use a TypeVar-parameterized generic as a `NewType` base");
    }

    // Known abstract protocols
    if KNOWN_PROTOCOLS.contains(&base) {
        return Some("cannot use a Protocol class as a `NewType` base");
    }

    // TypedDict subclass
    if typeddict_names.contains(&base) {
        return Some("cannot use a `TypedDict` class as a `NewType` base");
    }

    None
}

fn has_top_level_union(s: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'|' if depth == 0 => return true,
            _ => {}
        }
        i += 1;
    }
    s.starts_with("Union[")
}

fn is_typevar_parameterized_subscript(s: &str) -> bool {
    let bracket_pos = match s.find('[') {
        Some(p) => p,
        None => return false,
    };
    let base_name = s[..bracket_pos].trim();
    if !base_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    let inner_start = bracket_pos + 1;
    let inner_end = s.rfind(']').unwrap_or(s.len());
    if inner_end <= inner_start {
        return false;
    }
    let inner = s[inner_start..inner_end].trim();
    inner_has_typevar(inner)
}

fn inner_has_typevar(s: &str) -> bool {
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let part = s[start..i].trim();
                if looks_like_typevar(part) {
                    return true;
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    looks_like_typevar(last)
}

fn looks_like_typevar(s: &str) -> bool {
    let s = s.trim();
    if s.len() == 1 && s.chars().next().is_some_and(|c| c.is_uppercase()) {
        return true;
    }
    if s.starts_with(|c: char| c.is_uppercase())
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return true;
    }
    false
}

fn check_newtype_call(
    info: &NewTypeCallInfo,
    source: &str,
    path: &str,
    typeddict_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Too many arguments
    if info.positional_arg_count > 2 {
        diagnostics.push(make_diagnostic(
            format!(
                "`NewType` takes exactly 2 arguments ({} given) for `{}`",
                info.positional_arg_count, info.lhs_name
            ),
            info.span,
            path,
        ));
        return;
    }

    // Name mismatch: first arg string != LHS name
    if let Some(declared) = &info.declared_name {
        if *declared != info.lhs_name {
            diagnostics.push(make_diagnostic(
                format!(
                    "`NewType` name `{declared}` does not match the variable name `{}`",
                    info.lhs_name
                ),
                info.span,
                path,
            ));
        }
    }

    // Validate base type
    if let Some(base_text) = span_text(source, info.base_type_span) {
        if let Some(reason) = is_invalid_base(base_text.trim(), typeddict_names) {
            diagnostics.push(make_diagnostic(
                format!("Invalid base type for `NewType` `{}`: {reason}", info.lhs_name),
                info.span,
                path,
            ));
        }
    }
}

/// Emits BSK-E0050 for invalid `NewType(...)` calls.
pub(crate) struct InvalidNewType;

impl Rule for InvalidNewType {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        let typeddict_names: Vec<&str> = module
            .classes
            .iter()
            .filter(|c| c.is_typed_dict)
            .map(|c| c.name.as_str())
            .collect();

        for info in &module.newtype_calls {
            check_newtype_call(info, source, path, &typeddict_names, diagnostics);
        }
    }
}
