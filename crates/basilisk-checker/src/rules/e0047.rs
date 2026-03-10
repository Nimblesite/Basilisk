//! BSK-E0047: Invalid type expression in annotation.
//!
//! PEP 484 requires that annotations contain valid type expressions.
//! Only certain expression forms are valid as types:
//!
//! - Names (`int`, `str`, `MyClass`)
//! - Subscripts (`list[int]`, `dict[str, int]`)
//! - Binary-or unions (`int | str`)
//! - String literals (forward references)
//! - `None`
//! - `...` (Ellipsis, in Callable signatures)
//!
//! The following are invalid and should be flagged:
//!
//! - List literals: `[int, str]`
//! - Dict literals: `{}`
//! - Tuple literals: `(int, str)`
//! - List comprehensions: `[int for i in range(1)]`
//! - Lambda expressions (called or uncalled)
//! - Conditional expressions: `int if cond else str`
//! - Boolean binary operators: `int or str`, `int and str`
//! - F-string literals: `f"int"`
//! - Explicit function calls like `eval(...)`
//! - Negative numeric literals (positive are caught by E0024)
//! - Names that refer to module objects (`import types` → `types` is a module, not a type)
//! - Names that refer to unannotated literal variables (`var1 = 3` → `var1` is `int`, not a type)
//!
//! ```python
//! def f(x: [int, str]): ...            # E — list literal not a type
//! def g(x: int if True else str): ...  # E — conditional not a type
//! y: {} = {}                            # E — dict literal not a type
//! ```

use std::collections::HashSet;

use basilisk_resolver::{ImportKind, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0047",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0047",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "Type annotations must be valid type expressions (class names, subscripts, unions)"
                .to_owned(),
        ),
        note: Some(
            "PEP 484: annotations should be types, not arbitrary runtime expressions".to_owned(),
        ),
    }
}

/// Returns `true` when the annotation text is a structurally invalid type expression.
fn is_invalid_type_annotation(ann: &str) -> bool {
    let ann = ann.trim();

    if ann.is_empty() {
        return false;
    }

    // Handle string literal annotations (forward references)
    // If the annotation is a string literal, check the content inside
    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    // `Annotated[...]` annotations are validated by E0045, which checks the first argument
    // and enforces the minimum-two-arguments rule.  The metadata arguments (2nd+) may be
    // arbitrary runtime values including lambdas, calls, and literals, so checking the full
    // `Annotated[...]` text here would produce false positives.
    if content_to_check.starts_with("Annotated[") {
        return false;
    }

    // `Generic[T]` or bare `Generic` used as a type annotation is always invalid.
    // `Generic` is only meaningful in class base lists, not as a value/parameter type.
    if content_to_check == "Generic" || content_to_check.starts_with("Generic[") {
        return true;
    }

    // List literal or list comprehension: starts with `[`
    if content_to_check.starts_with('[') {
        return true;
    }

    // Dict literal: starts with `{`
    if content_to_check.starts_with('{') {
        return true;
    }

    // F-string: starts with f" or f'
    if content_to_check.starts_with("f\"") || content_to_check.starts_with("f'") {
        return true;
    }

    // Numeric literal (positive or negative): 1, -1, 3.14
    // Positive numerics as direct annotations are caught by E0024, but when they appear
    // inside string annotations (e.g. `"1"`, `"3.14"`) E0024 does not fire.
    if content_to_check.starts_with('-')
        && content_to_check[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    // Positive numeric literal inside a string annotation: `"1"`, `"42"`, `"3.14"`
    if !content_to_check.is_empty()
        && content_to_check
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        && content_to_check
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    // Boolean literal used as a type annotation: `"True"`, `"False"`
    if content_to_check == "True" || content_to_check == "False" {
        return true;
    }

    // Conditional expression: ` if ` keyword at depth 0
    if has_top_level_token(content_to_check, " if ") {
        return true;
    }

    // Boolean binary operators: ` or ` / ` and ` at depth 0
    // Note: `|` is valid (union), `or`/`and` keywords are not
    if has_top_level_token(content_to_check, " or ")
        || has_top_level_token(content_to_check, " and ")
    {
        return true;
    }

    // Tuple literal: `(int, str)` — parens with comma at depth 0
    if content_to_check.starts_with('(')
        && content_to_check.ends_with(')')
        && paren_contains_top_level_comma(content_to_check)
    {
        return true;
    }

    // Lambda (possibly called)
    if content_to_check.contains("lambda") {
        return true;
    }

    // Explicit eval() call
    if content_to_check.starts_with("eval(") {
        return true;
    }

    // String literal used as an operand in a `|` union expression.
    // e.g. `"ClassA" | int` or `int | "ClassA"` — causes a runtime TypeError.
    // Valid form would be `"ClassA | int"` (entire union as a string) or `Union["ClassA", int]`.
    if has_string_literal_in_pipe_union(content_to_check) {
        return true;
    }

    false
}

/// Returns `true` when the text contains a `|` union at depth 0 where one of the
/// pipe-separated parts is a quoted string literal (a misused forward reference).
fn has_string_literal_in_pipe_union(s: &str) -> bool {
    // Walk through and collect top-level | splits
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut string_char = b'"';
    let mut part_start = 0usize;

    let mut parts: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if in_string {
            if ch == string_char && (i == 0 || bytes[i - 1] != b'\\') {
                in_string = false;
            }
        } else {
            match ch {
                b'"' | b'\'' => {
                    in_string = true;
                    string_char = ch;
                }
                b'[' | b'(' | b'{' => depth += 1,
                b']' | b')' | b'}' => depth -= 1,
                b'|' if depth == 0 => {
                    parts.push(s[part_start..i].trim());
                    part_start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    if parts.is_empty() {
        return false; // no top-level | found
    }
    parts.push(s[part_start..].trim());

    parts.iter().any(|part| {
        let p = part.trim();
        (p.starts_with('"') && p.ends_with('"') && p.len() >= 2)
            || (p.starts_with('\'') && p.ends_with('\'') && p.len() >= 2)
    })
}

/// Returns `true` when the text contains `token` at bracket depth 0.
fn has_top_level_token(s: &str, token: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let tok = token.as_bytes();
    let tok_len = tok.len();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            _ if depth == 0 => {
                if bytes.get(i..i + tok_len) == Some(tok) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Returns `true` when `(...)` contains a comma at depth 0 inside the parens.
fn paren_contains_top_level_comma(s: &str) -> bool {
    let inner = &s[1..s.len() - 1];
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Build a set of names that are definitely not valid type expressions:
/// - Names bound to modules via plain `import X` statements (the module object is not a type).
/// - Names bound to unannotated simple literal values (`var1 = 3` → `var1` is `int`, not a type).
fn collect_non_type_names(module: &ResolvedModule) -> HashSet<String> {
    let mut names = HashSet::new();

    // Plain `import X` binds `X` to a module object in scope.
    for import in &module.imports {
        if import.kind == ImportKind::Plain {
            // `import os.path` binds `os` (first component). For `import types`, binding is `types`.
            let local_name = import
                .module
                .split('.')
                .next_back()
                .unwrap_or(import.module.as_str());
            names.insert(local_name.to_owned());
        }
    }

    // Unannotated module-level variables with simple literal RHS are not types.
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let is_simple_literal = matches!(
            var.rhs_kind,
            RhsKind::IntLiteral
                | RhsKind::FloatLiteral
                | RhsKind::StrLiteral
                | RhsKind::BoolLiteral
                | RhsKind::BytesLiteral
                | RhsKind::EmptyList
                | RhsKind::EmptyDict
                | RhsKind::NoneValue
        );
        if is_simple_literal {
            names.insert(var.name.clone());
        }
    }

    names
}

/// Returns `true` when the annotation text is exactly a name bound to a non-type in module scope.
fn is_non_type_name(ann: &str, non_type_names: &HashSet<String>) -> bool {
    let ann = ann.trim();

    // Handle string literal annotations (forward references)
    // If the annotation is a string literal, check the content inside
    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    // Only match bare identifiers — no subscripts, dot access, or call expressions.
    if content_to_check.contains('[')
        || content_to_check.contains('.')
        || content_to_check.contains('(')
        || content_to_check.contains(' ')
    {
        return false;
    }
    non_type_names.contains(content_to_check)
}

/// Returns `true` when the annotation uses a `ParamSpec` in an invalid position.
///
/// Valid positions for `P` (a `ParamSpec`):
/// - As the parameters argument of `Callable`: `Callable[P, ReturnType]`
/// - Inside `Concatenate` as the LAST argument: `Concatenate[T, P]` inside Callable
/// - As a type parameter in `Generic[P]`
///
/// Invalid positions (detected here):
/// - Bare `P` as a direct annotation
/// - `Concatenate[...]` used outside of `Callable`
/// - `P` inside a non-Callable subscript: `list[P]`, `dict[str, P]`
/// - `P` as the return type of `Callable`: `Callable[[int, str], P]`
fn is_paramspec_invalid_annotation(ann: &str, paramspec_names: &HashSet<&str>) -> bool {
    let ann = ann.trim();
    if ann.is_empty() || paramspec_names.is_empty() {
        return false;
    }

    // Case 1: bare ParamSpec name used as a direct annotation.
    if paramspec_names.contains(ann) {
        return true;
    }

    // Case 2: `Concatenate[...]` used outside of a `Callable` context.
    // When `Concatenate` appears as a direct annotation (not inside `Callable[...]`),
    // it's always invalid.
    if ann.starts_with("Concatenate[") {
        return true;
    }

    // For the remaining cases, we only check subscript annotations.
    if !ann.contains('[') {
        return false;
    }

    // Case 3: ParamSpec inside a non-Callable subscript (e.g. `list[P]`).
    // If the annotation starts with something other than `Callable[`, look for ParamSpec names.
    if !ann.starts_with("Callable[") {
        for name in paramspec_names {
            if ann.contains(name) {
                // Make sure it's not just a substring of a longer identifier.
                // Check that the name appears surrounded by non-identifier chars.
                let name_len = name.len();
                let ann_bytes = ann.as_bytes();
                for start in 0..ann.len().saturating_sub(name_len - 1) {
                    if ann[start..].starts_with(name) {
                        let end = start + name_len;
                        let before_ok = start == 0
                            || !ann_bytes[start - 1].is_ascii_alphanumeric()
                                && ann_bytes[start - 1] != b'_';
                        let after_ok = end >= ann.len()
                            || !ann_bytes[end].is_ascii_alphanumeric() && ann_bytes[end] != b'_';
                        if before_ok && after_ok {
                            return true;
                        }
                    }
                }
            }
        }
        return false;
    }

    // Case 4: `Callable[[int, str], P]` — ParamSpec as the return type of Callable.
    // The return type is the last top-level argument to Callable[...].
    // We detect this by finding the last top-level comma, then checking if what follows is a ParamSpec name.
    let inner = ann.trim_start_matches("Callable[").trim_end_matches(']');
    let last_arg = last_top_level_arg(inner);
    if let Some(last) = last_arg {
        let last_trimmed = last.trim();
        if paramspec_names.contains(last_trimmed) {
            return true;
        }
    }

    false
}

/// Python built-in type names that are always valid as forward references in annotations.
/// Used to avoid false positives in the circular reference check.
const PYTHON_BUILTIN_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "complex",
    "bytearray",
    "memoryview",
    "object",
    "type",
    "None",
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "range",
    "slice",
    "super",
    "classmethod",
    "staticmethod",
    "property",
    "Exception",
    "BaseException",
    "ValueError",
    "TypeError",
    "AttributeError",
    "KeyError",
    "IndexError",
    "RuntimeError",
    "StopIteration",
    "NotImplementedError",
    "OSError",
    "IOError",
    "FileNotFoundError",
    "PermissionError",
    "TimeoutError",
    "ConnectionError",
    "ArithmeticError",
    "OverflowError",
    "ZeroDivisionError",
    "ImportError",
    "ModuleNotFoundError",
    "NameError",
    "UnboundLocalError",
    "LookupError",
    "SyntaxError",
    "SystemExit",
    "KeyboardInterrupt",
    "GeneratorExit",
    "UnicodeError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "Any",
    "Union",
    "Optional",
    "Callable",
    "Tuple",
    "List",
    "Dict",
    "Set",
    "FrozenSet",
    "Type",
    "ClassVar",
    "Final",
    "Literal",
    "TypeVar",
    "Generic",
    "Protocol",
    "TypedDict",
    "NamedTuple",
    "Annotated",
    "TypeAlias",
    "TypeGuard",
    "ParamSpec",
    "TypeVarTuple",
    "Concatenate",
    "Never",
    "LiteralString",
    "Self",
    "Unpack",
    "overload",
    "cast",
    "assert_type",
    "reveal_type",
    "Iterable",
    "Iterator",
    "Generator",
    "Sequence",
    "Mapping",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "Coroutine",
    "AsyncIterator",
    "AsyncIterable",
    "AsyncGenerator",
    "Pattern",
    "Match",
    "IO",
    "TextIO",
    "BinaryIO",
    "AbstractSet",
];

/// Returns `true` when `ann` is a bare identifier (only alphanumeric chars and underscores,
/// no string quotes, brackets, dots, parens, spaces, or operators).
fn is_bare_identifier(ann: &str) -> bool {
    !ann.is_empty()
        && !ann.starts_with('"')
        && !ann.starts_with('\'')
        && ann.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Returns `true` when the annotation is a string literal whose inner content equals
/// the attribute name, and that name is not defined in the module scope or as a Python
/// built-in type.
///
/// This detects circular forward references like `ClassF: "ClassF"` when `ClassF` has
/// no other definition reachable at module scope.
fn is_circular_string_annotation(
    ann: &str,
    attr_name: &str,
    module_scope_names: &HashSet<&str>,
    builtin_names: &HashSet<&str>,
) -> bool {
    let content = if (ann.starts_with('"') && ann.ends_with('"') && ann.len() >= 2)
        || (ann.starts_with('\'') && ann.ends_with('\'') && ann.len() >= 2)
    {
        &ann[1..ann.len() - 1]
    } else {
        return false;
    };
    content == attr_name
        && !module_scope_names.contains(content)
        && !builtin_names.contains(content)
}

/// Return the last top-level comma-separated argument from a subscript's inner text.
fn last_top_level_arg(inner: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut last_comma = None;
    let bytes = inner.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    last_comma.map(|pos| &inner[pos + 1..])
}

/// Emits BSK-E0047 when an annotation contains an invalid type expression.
pub(crate) struct InvalidTypeAnnotation;

impl Rule for InvalidTypeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        check_invalid_type_annotations(module, diagnostics);
    }
}

fn build_module_scope_names<'a>(module: &'a ResolvedModule) -> HashSet<&'a str> {
    let mut names: HashSet<&'a str> = HashSet::new();
    for cls in &module.classes {
        names.insert(cls.name.as_str());
    }
    for var in &module.module_vars {
        names.insert(var.name.as_str());
    }
    for imp in &module.imports {
        match imp.kind {
            ImportKind::From => {
                for name in &imp.names {
                    names.insert(name.as_str());
                }
            }
            ImportKind::Plain => {
                if let Some(name) = imp.module.split('.').next() {
                    names.insert(name);
                }
            }
            ImportKind::Star => {}
        }
    }
    names
}

fn check_function_param_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    paramspec_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if param.annotation_is_numeric_literal {
                continue;
            }
            let Some(ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
                || is_paramspec_invalid_annotation(ann_trimmed, paramspec_names)
            {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for parameter `{}`",
                        param.name
                    ),
                    param.name_span,
                    path,
                ));
            }
        }
    }
}

fn check_module_var_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    paramspec_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for var in &module.module_vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        let ann_trimmed = ann.trim();
        if is_invalid_type_annotation(ann_trimmed) || is_non_type_name(ann_trimmed, non_type_names)
        {
            diagnostics.push(make_diagnostic(
                format!("Invalid type expression in annotation for `{}`", var.name),
                var.name_span,
                path,
            ));
            continue;
        }
        if ann_trimmed == "TypeAlias" {
            if let Some(rhs) = span_text(source, var.rhs_span) {
                let rhs_trimmed = rhs.trim();
                if paramspec_names.contains(rhs_trimmed) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`TypeAlias` `{}` has a `ParamSpec` as its type, which is invalid; \
                             `ParamSpec` can only be used in `Callable[P, ReturnType]`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }
    }
}

fn check_local_var_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for func in &module.functions {
        for var in &func.local_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
            {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for local variable `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }
    }
}

fn check_class_attr_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    module_scope_names: &HashSet<&str>,
    builtin_type_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for cls in &module.classes {
        let cls_method_names: HashSet<&str> = cls.method_names.iter().map(String::as_str).collect();
        for attr in &cls.attributes {
            let Some(ann) = span_text(source, attr.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
            {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
                continue;
            }
            if is_bare_identifier(ann_trimmed) && cls_method_names.contains(ann_trimmed) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
                continue;
            }
            if is_circular_string_annotation(
                ann_trimmed,
                &attr.name,
                module_scope_names,
                builtin_type_names,
            ) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
            }
        }
    }
}

fn check_invalid_type_annotations(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let non_type_names = collect_non_type_names(module);
    let module_scope_names = build_module_scope_names(module);
    let builtin_type_names: HashSet<&str> = PYTHON_BUILTIN_TYPE_NAMES.iter().copied().collect();
    let paramspec_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .filter(|tv| tv.is_paramspec)
        .map(|tv| tv.name.as_str())
        .collect();

    check_function_param_annotations(module, &non_type_names, &paramspec_names, diagnostics);
    check_module_var_annotations(module, &non_type_names, &paramspec_names, diagnostics);
    check_local_var_annotations(module, &non_type_names, diagnostics);
    check_class_attr_annotations(
        module,
        &non_type_names,
        &module_scope_names,
        &builtin_type_names,
        diagnostics,
    );
}
