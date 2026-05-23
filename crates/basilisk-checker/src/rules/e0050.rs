//! Implements [BSK-E0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-structural
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

use crate::diagnostic::{error_diagnostic, error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0050",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0050",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("`NewType` requires exactly two arguments: a string name and a concrete base class"),
        Some("PEP 484: `NewType` accepts only proper concrete classes as the base type"),
    )
}

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    slice_span(source, span)
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
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(b'|') if depth == 0 => return true,
            _ => {}
        }
        i += 1;
    }
    s.starts_with("Union[")
}

fn is_typevar_parameterized_subscript(s: &str) -> bool {
    let Some(bracket_pos) = s.find('[') else {
        return false;
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
    // Wrong number of arguments
    if info.positional_arg_count != 2 {
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
                format!(
                    "Invalid base type for `NewType` `{}`: {reason}",
                    info.lhs_name
                ),
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

        let typeddict_names: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.classes, |c| c.is_typed_dict);

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

        // Build map: newtype_name → base_type_text
        let newtype_base: std::collections::HashMap<&str, &str> = module
            .newtype_calls
            .iter()
            .filter_map(|nt| {
                let base_text = span_text(source, nt.base_type_span)?;
                Some((nt.lhs_name.as_str(), base_text.trim()))
            })
            .collect();

        check_newtype_subclassing(module, &newtype_names, diagnostics);
        check_newtype_subscript_uses(module, source, path, &newtype_names, diagnostics);
        check_newtype_assigned_to_type(module, source, path, &newtype_names, diagnostics);
        check_isinstance_with_newtype(module, source, path, &newtype_names, diagnostics);
        check_newtype_call_arg_types(module, source, path, &newtype_base, diagnostics);
        check_newtype_var_literal_assignments(module, source, path, &newtype_names, diagnostics);
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
        let Some(rhs_text) = slice_span(source, rhs_span) else {
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
        let Some(arg_text) = slice_span(source, *second_span) else {
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
    let Some(bracket_pos) = ann.find('[') else {
        return false;
    };
    let name_part = ann[..bracket_pos].trim();
    newtype_names.contains(name_part)
}

/// Check calls to `NewType` constructors for argument type mismatches.
///
/// `UserId("user")` when `UserId = NewType("UserId", int)` → error because `str` ≠ `int`.
fn check_newtype_call_arg_types(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    newtype_base: &std::collections::HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        let Some(&base_type) = newtype_base.get(call.callee.as_str()) else {
            continue;
        };

        let Some((rhs_kind, arg_span)) = call.args.first() else {
            continue;
        };

        if let Some(_description) = newtype_arg_mismatch(base_type, rhs_kind) {
            let Some(arg_text) = slice_span(source, *arg_span) else {
                continue;
            };
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument to `{}` ({}) is not compatible with its base type `{base_type}`",
                    call.callee,
                    arg_text.trim()
                ),
                call.span,
                path,
                Some(format!(
                    "Pass a value of type `{base_type}` to the `{}` constructor",
                    call.callee
                )),
                Some("NewType constructors accept only values of the base type".to_owned()),
            ));
        }
    }
}

/// Returns a description of the mismatch when `rhs` is incompatible with `base_type`, else `None`.
fn newtype_arg_mismatch(base_type: &str, rhs: &basilisk_resolver::RhsKind) -> Option<&'static str> {
    use basilisk_resolver::RhsKind;

    let base = base_type.trim().to_ascii_lowercase();
    match (base.as_str(), rhs) {
        ("int" | "float" | "bool" | "bytes", RhsKind::StrLiteral) => Some("str literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("bytes literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("float literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("int literal"),
        _ => None,
    }
}

/// Check module-level variable assignments where the annotation is a `NewType` name.
///
/// `u1: UserId = 42` is wrong because plain `int` literals are not `UserId` values.
/// Only `UserId(42)` creates a proper `UserId`.
fn check_newtype_var_literal_assignments(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    newtype_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use basilisk_resolver::RhsKind;

    for var in &module.module_vars {
        let Some(ann_text) = span_text(source, var.annotation_span) else {
            continue;
        };
        let ann = ann_text.trim();
        if !newtype_names.contains(ann) {
            continue;
        }

        // A literal value can never be a NewType instance — you must call the constructor.
        let is_bare_literal = matches!(
            var.rhs_kind,
            RhsKind::IntLiteral
                | RhsKind::FloatLiteral
                | RhsKind::StrLiteral
                | RhsKind::BytesLiteral
                | RhsKind::BoolLiteral
                | RhsKind::NoneValue
        );

        if is_bare_literal {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot assign a literal value directly to `{ann}`; \
                     use `{ann}(value)` to create a `{ann}` instance"
                ),
                var.name_span,
                path,
                Some(format!("Replace the literal with `{ann}(value)`")),
                Some(
                    "NewType creates a distinct type; only the constructor call is valid"
                        .to_owned(),
                ),
            ));
        }
    }
}
