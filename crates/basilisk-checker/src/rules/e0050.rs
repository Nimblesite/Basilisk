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
    let Some(bracket_pos) = s.find('[') else { return false };
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
    if s.len() == 1 && s.chars().next().is_some_and(char::is_uppercase) {
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

        // Collect all NewType names defined in this module.
        let newtype_names: std::collections::HashSet<&str> = module
            .newtype_calls
            .iter()
            .map(|nt| nt.lhs_name.as_str())
            .collect();

        if newtype_names.is_empty() {
            return;
        }

        check_newtype_subclassing(module, &newtype_names, diagnostics);
        check_newtype_subscript_uses(module, source, path, &newtype_names, diagnostics);
        check_newtype_assigned_to_type(module, source, path, &newtype_names, diagnostics);
        check_isinstance_with_newtype(module, source, path, &newtype_names, diagnostics);
    }
}

/// Subclassing a `NewType` is not allowed (PEP 484).
fn check_newtype_subclassing(
    module: &ResolvedModule,
    newtype_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for cls in &module.classes {
        for base in &cls.bases {
            if newtype_names.contains(base.as_str()) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Class `{}` cannot subclass `{}` which is a `NewType`",
                        cls.name, base
                    ),
                    cls.def_span,
                    path,
                ));
            }
        }
    }
}

/// Using a `NewType` as a generic subscript (`MyNewType[int]`) is not allowed.
fn check_newtype_subscript_uses(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    newtype_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if let Some(ann) = span_text(source, param.annotation_span) {
                if is_newtype_subscript(ann.trim(), newtype_names) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Parameter `{}`: `NewType` cannot be used as a generic type",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
        }
    }
    for var in &module.module_vars {
        if let Some(ann) = span_text(source, var.annotation_span) {
            if is_newtype_subscript(ann.trim(), newtype_names) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Variable `{}`: `NewType` cannot be used as a generic type",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }
    }
    for cls in &module.classes {
        for attr in &cls.attributes {
            if let Some(ann) = span_text(source, attr.annotation_span) {
                if is_newtype_subscript(ann.trim(), newtype_names) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Attribute `{}`: `NewType` cannot be used as a generic type",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }
            }
        }
    }
}

/// `_: type = UserId` — assigning a `NewType` to a `type`-annotated variable is invalid.
///
/// PEP 484: `NewType(...)` does not return a class object; it returns a callable.
fn check_newtype_assigned_to_type(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    newtype_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(ann_text) = span_text(source, var.annotation_span) else {
            continue;
        };
        if ann_text.trim() != "type" {
            continue;
        }
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = source.get(rhs_span.start as usize..rhs_span.end as usize) else {
            continue;
        };
        if newtype_names.contains(rhs_text.trim()) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a `NewType`, not an instance of `type`; \
                     `NewType()` does not return a class object",
                    rhs_text.trim()
                ),
                var.name_span,
                path,
            ));
        }
    }
}

/// `isinstance(u2, UserId)` — using a `NewType` as the second argument to `isinstance` is invalid.
///
/// PEP 484: the object returned by `NewType(...)` is not a class and cannot be
/// used as the second argument to `isinstance` or `issubclass`.
fn check_isinstance_with_newtype(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    newtype_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        if call.callee != "isinstance" {
            continue;
        }
        let Some((_, second_span)) = call.args.get(1) else {
            continue;
        };
        let Some(arg_text) = source.get(second_span.start as usize..second_span.end as usize)
        else {
            continue;
        };
        if newtype_names.contains(arg_text.trim()) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a `NewType` and cannot be used as the second argument \
                     to `isinstance`; `NewType` types are not runtime classes",
                    arg_text.trim()
                ),
                call.span,
                path,
            ));
        }
    }
}

/// Returns `true` if the annotation text looks like `NewTypeName[...]`.
fn is_newtype_subscript(ann: &str, newtype_names: &std::collections::HashSet<&str>) -> bool {
    let Some(bracket_pos) = ann.find('[') else { return false };
    let name_part = ann[..bracket_pos].trim();
    newtype_names.contains(name_part)
}
