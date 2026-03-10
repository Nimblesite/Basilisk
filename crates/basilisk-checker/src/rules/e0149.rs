//! BSK-E0149: PEP 695 generic type parameter scoping violations.
//!
//! Detects violations of PEP 695 type parameter scoping rules:
//!
//! 1. A type parameter's bound references another type parameter in the same
//!    parameter list that has not yet been defined (forward reference in bounds).
//!    Per PEP 695: "A compiler error or runtime exception is generated if the
//!    definition of an earlier type parameter references a later type parameter."
//!
//! 2. A PEP 695 type parameter is used at module level or in a decorator
//!    applied to a generic construct, outside the scope where the type parameter
//!    is defined.
//!
//! 3. A method inside a generic class defines its own type parameter with the
//!    same name as the enclosing class's type parameter, creating a shadowing
//!    conflict.
//!
//! ```python
//! class ClassA[S, T: Sequence[S]]:  # E — T's bound references S (earlier param)
//!     ...
//!
//! class ClassB[S: Sequence[T], T]:  # E — S's bound references T (later param)
//!     ...
//!
//! print(T)  # E — T is not defined at module scope
//!
//! @decorator(Foo[T])  # E — T not in scope in decorator
//! class ClassD[T]: ...
//!
//! class ClassE[T]:
//!     def method1[T](self): ...  # E — method re-defines class type param T
//! ```
//!
//! Reference: <https://peps.python.org/pep-0695/#type-parameter-scopes>

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0149",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0149",
};

/// Emits BSK-E0149 for PEP 695 generic type parameter scoping violations.
pub(crate) struct Pep695TypeParamScopingViolation;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the leading whitespace (indentation) of a line.
fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Return the byte offset (as u32) of the start of the given 1-based line.
#[allow(clippy::cast_possible_truncation)]
fn line_start_offset(source: &str, target_line: usize) -> u32 {
    let mut current = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current += 1;
        }
    }
    source.len() as u32
}

/// Build a `Span` covering the trimmed content of a given 1-based line.
#[allow(clippy::cast_possible_truncation)]
fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_start_offset(source, line_number) as usize;
    let line_text = source[start..].lines().next().unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

/// Check whether `name` appears as a whole identifier in `text`.
fn contains_name(text: &str, name: &str) -> bool {
    let needle = name.as_bytes();
    let haystack = text.as_bytes();
    let nlen = needle.len();
    if nlen > haystack.len() {
        return false;
    }
    haystack.windows(nlen).enumerate().any(|(idx, window)| {
        if window != needle {
            return false;
        }
        let before_ok =
            idx == 0 || (!haystack[idx - 1].is_ascii_alphanumeric() && haystack[idx - 1] != b'_');
        let after_ok = idx + nlen >= haystack.len()
            || (!haystack[idx + nlen].is_ascii_alphanumeric() && haystack[idx + nlen] != b'_');
        before_ok && after_ok
    })
}

/// Extract PEP 695 type parameter names from a `[...]` type parameter list
/// on a `class` or `def` line.
///
/// Returns `None` if there is no `[...]` list.
/// Returns `Some(vec)` with the ordered parameter names if found.
fn extract_pep695_type_params(line: &str) -> Option<Vec<String>> {
    // Find the opening `[` that is part of a type parameter list.
    // It must appear before `(` (for class) or `:` (for def).
    let bracket_start = line.find('[')?;
    let colon_or_paren = line.find('(').unwrap_or(line.len());
    let colon_pos = line.find(':').unwrap_or(line.len());
    let first_end = colon_or_paren.min(colon_pos);

    // The `[` must appear before `(` or `:`.
    if bracket_start >= first_end {
        return None;
    }

    // Find the matching `]`.
    let after_bracket = &line[bracket_start + 1..];
    let mut depth = 1usize;
    let mut end_idx = None;
    for (i, ch) in after_bracket.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end_idx = end_idx?;
    let params_text = &after_bracket[..end_idx];

    // Split on commas at depth 0 to get individual parameter specs.
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for ch in params_text.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                let param = current.trim().to_owned();
                if !param.is_empty() {
                    params.push(param);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let last = current.trim().to_owned();
    if !last.is_empty() {
        params.push(last);
    }

    // Extract just the name from each param spec.
    // Specs can be: `T`, `T: Bound`, `**P`, `*Ts`
    let names: Vec<String> = params
        .iter()
        .map(|spec| {
            let s = spec.trim_start_matches('*');
            // Take everything before `:` as the name.
            s.split(':').next().unwrap_or(s).trim().to_owned()
        })
        .filter(|n| !n.is_empty())
        .collect();

    Some(names)
}

/// Extract the bound portion of a type param spec like `T: Sequence[S]`.
/// Returns the text after the `:` if present.
fn extract_bound(param_spec: &str) -> Option<&str> {
    let colon_pos = param_spec.find(':')?;
    Some(param_spec[colon_pos + 1..].trim())
}

// ---------------------------------------------------------------------------
// Violation 1: forward / backward references in PEP 695 type param bounds
// ---------------------------------------------------------------------------

/// Check whether a PEP 695 type parameter list has bound violations where a
/// param's bound references another param in the same list.
///
/// Per PEP 695: referencing *any* other type param in the same list from a
/// bound is an error (whether earlier or later).
fn check_pep695_bound_cross_references(
    line: &str,
    line_number: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed = line.trim();

    // Only apply to `class` or `def` lines.
    let is_class = trimmed.starts_with("class ");
    let is_def = trimmed.starts_with("def ") || trimmed.starts_with("async def ");
    if !is_class && !is_def {
        return;
    }

    let Some(param_names) = extract_pep695_type_params(trimmed) else {
        return;
    };
    if param_names.len() < 2 {
        return;
    }

    // Find the raw params text again so we can get bound specs.
    let bracket_start = trimmed.find('[').unwrap_or(trimmed.len());
    let after_bracket = &trimmed[bracket_start + 1..];
    let mut depth = 1usize;
    let mut end_idx = 0;
    for (i, ch) in after_bracket.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let params_text = &after_bracket[..end_idx];

    // Parse raw param specs (including bounds).
    let mut raw_specs: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for ch in params_text.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                raw_specs.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    raw_specs.push(current.trim().to_owned());

    // For each param spec that has a bound, check if the bound references any
    // other param name in the list.
    let all_names: Vec<&str> = param_names.iter().map(String::as_str).collect();

    for (idx, spec) in raw_specs.iter().enumerate() {
        let Some(bound_text) = extract_bound(spec) else {
            continue;
        };

        // Check if the bound references any other param in the list.
        for (other_idx, other_name) in all_names.iter().enumerate() {
            if other_idx == idx {
                continue;
            }
            if contains_name(bound_text, other_name) {
                let param_name = &all_names[idx];
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "PEP 695 type parameter `{param_name}` bound references \
                         `{other_name}` from the same type parameter list"
                    ),
                    span: span_for_line(source, line_number),
                    path: path.to_owned(),
                    help: Some(
                        "Type parameter bounds cannot reference other type parameters \
                         in the same list"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 695: a compiler error is generated if the definition of \
                         a type parameter references another type parameter in the \
                         same list"
                            .to_owned(),
                    ),
                });
                // Emit one diagnostic per violating param (not per cross-ref).
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 2: PEP 695 type params used outside their scope
// ---------------------------------------------------------------------------

/// Collect all PEP 695 type parameter names defined in the file.
/// Returns a list of (`param_name``defining_line_number`er).
fn collect_pep695_type_params(source: &str) -> Vec<(String, usize)> {
    let mut result = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_number = idx + 1;
        let trimmed = line.trim();
        if !trimmed.starts_with("class ")
            && !trimmed.starts_with("def ")
            && !trimmed.starts_with("async def ")
            && !trimmed.starts_with("type ")
        {
            continue;
        }
        if let Some(names) = extract_pep695_type_params(trimmed) {
            for name in names {
                result.push((name, line_number));
            }
        }
    }
    result
}

/// Check whether a name has been assigned at module scope (indent == 0) in
/// any of the lines before `before_line` (1-based).
///
/// A simple assignment like `T = ...` or `T: type = ...` counts as a prior
/// binding that shadows any PEP 695 type param of the same name.
fn has_prior_module_assignment(lines: &[&str], name: &str, before_line: usize) -> bool {
    for line in lines.iter().take(before_line.saturating_sub(1)) {
        if leading_indent(line) != 0 {
            continue;
        }
        let trimmed = line.trim();
        // `name = ...` or `name: annotation = ...`
        if trimmed.starts_with(&format!("{name} "))
            || trimmed.starts_with(&format!("{name}="))
            || trimmed.starts_with(&format!("{name}:"))
        {
            // Ensure it is an assignment, not a class/def header or similar.
            // The presence of `=` in the line (after the name) is enough.
            if trimmed.contains('=') {
                return true;
            }
        }
    }
    false
}

/// Check whether a decorator line uses a PEP 695 type param that belongs to
/// the class/def immediately following the decorator.
///
/// Decorators are evaluated *before* the class/def scope is entered, so the
/// class's own type params are not yet available.
///
/// However, if the name was already assigned at module scope before the
/// decorator, the decorator is using that module-level variable, not the
/// class's type parameter, and no violation is emitted.
fn check_decorator_uses_class_type_param(
    lines: &[&str],
    decorator_line: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the class or def that the decorator applies to (next non-decorator
    // non-blank line after the decorator line).
    let mut target_line: Option<usize> = None;
    for (idx, line) in lines.iter().enumerate().skip(decorator_line) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('@') {
            continue;
        }
        // This is the decorated definition.
        target_line = Some(idx + 1); // 1-based
        break;
    }

    let Some(target_lno) = target_line else {
        return;
    };

    let target_trimmed = lines[target_lno - 1].trim();

    // Collect type params of the target definition.
    let Some(target_params) = extract_pep695_type_params(target_trimmed) else {
        return;
    };
    if target_params.is_empty() {
        return;
    }

    // Check the decorator line for references to those type params.
    let decorator_text = lines[decorator_line - 1];
    for param_name in &target_params {
        if !contains_name(decorator_text, param_name) {
            continue;
        }

        // If the name was already assigned at module scope before this
        // decorator, it resolves to the module-level variable, not the
        // type parameter.  No violation in that case.
        if has_prior_module_assignment(lines, param_name, decorator_line) {
            continue;
        }

        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "PEP 695 type parameter `{param_name}` is not defined at \
                 this point: it belongs to the decorated definition, not \
                 the decorator call"
            ),
            span: span_for_line(source, decorator_line),
            path: path.to_owned(),
            help: Some(format!(
                "`{param_name}` is a type parameter of the class/function \
                 being decorated; it is not in scope in the decorator arguments"
            )),
            note: Some(
                "PEP 695: type parameter scopes are entered after the decorator \
                 expressions are evaluated"
                    .to_owned(),
            ),
        });
    }
}

/// Check whether a module-level non-definition line uses a PEP 695 type
/// parameter that is not in scope.
///
/// PEP 695 type params are only in scope inside the class/def body where they
/// are declared. Using them at module level (e.g. `print(T)`) is a runtime
/// error.
fn check_module_level_type_param_use(
    line: &str,
    line_number: usize,
    all_pep695_params: &[(String, usize)],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("class ")
        || trimmed.starts_with("def ")
        || trimmed.starts_with("async def ")
        || trimmed.starts_with("type ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with('@')
    {
        return;
    }

    // Only check module-level lines (indent == 0).
    if leading_indent(line) != 0 {
        return;
    }

    let before_comment = trimmed.split_once('#').map_or(trimmed, |(code, _)| code);

    for (param_name, _defining_line) in all_pep695_params {
        if contains_name(before_comment, param_name) {
            // Make sure this is not an assignment that defines a new binding
            // of the same name (e.g. `T = int(0)`).
            let is_assignment = before_comment.trim().starts_with(&format!("{param_name} "))
                || before_comment.trim().starts_with(&format!("{param_name}="))
                || before_comment.trim() == param_name.as_str();

            // Allow `T = ...` and `T: annotation = ...` assignments which
            // create a module-level binding and shadow the type param.
            if is_assignment {
                continue;
            }

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "PEP 695 type parameter `{param_name}` is not defined at \
                     module scope; it is only accessible inside the generic \
                     class or function where it is declared"
                ),
                span: span_for_line(source, line_number),
                path: path.to_owned(),
                help: Some(format!(
                    "`{param_name}` is a PEP 695 type parameter and is not \
                     bound at module scope"
                )),
                note: Some(
                    "PEP 695: type parameter names are only defined inside \
                     the body of the generic class or function"
                        .to_owned(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 3: method re-defines class type param with its own [T]
// ---------------------------------------------------------------------------

/// Inside a generic class body (`class Foo[T]`), check if any method
/// defines its own type parameter list that re-uses the class's type param
/// names.  Per PEP 695, this is a scoping violation.
fn check_method_redefines_class_type_param(
    lines: &[&str],
    class_line: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let class_trimmed = lines[class_line - 1].trim();
    let Some(class_params) = extract_pep695_type_params(class_trimmed) else {
        return;
    };
    if class_params.is_empty() {
        return;
    }

    let class_indent = leading_indent(lines[class_line - 1]);
    let method_indent = class_indent + 4; // standard Python indent

    // Walk subsequent lines that are at method indentation inside the class.
    for (idx, line) in lines.iter().enumerate().skip(class_line) {
        let line_number = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = leading_indent(line);

        // If indentation went back to class level or less, class body ended.
        if indent <= class_indent && !trimmed.is_empty() {
            break;
        }

        // Only look at direct method definitions inside the class.
        if indent != method_indent {
            continue;
        }
        if !trimmed.starts_with("def ") && !trimmed.starts_with("async def ") {
            continue;
        }

        let Some(method_params) = extract_pep695_type_params(trimmed) else {
            continue;
        };
        if method_params.is_empty() {
            continue;
        }

        // Check if any method type param re-uses a class type param name.
        for method_param in &method_params {
            if class_params.contains(method_param) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Method type parameter `{method_param}` shadows the enclosing \
                         class's type parameter of the same name"
                    ),
                    span: span_for_line(source, line_number),
                    path: path.to_owned(),
                    help: Some(format!(
                        "Rename the method's type parameter `{method_param}` to \
                         avoid shadowing the class type parameter"
                    )),
                    note: Some(
                        "PEP 695: a method that defines its own type parameter with \
                         the same name as an enclosing class type parameter creates \
                         a scoping violation"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule implementation
// ---------------------------------------------------------------------------

impl Rule for Pep695TypeParamScopingViolation {
    #[allow(clippy::too_many_lines)]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let lines: Vec<&str> = source.lines().collect();

        // Collect all PEP 695 type params defined anywhere in the file.
        let all_pep695_params = collect_pep695_type_params(source);

        for (line_idx, &line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // --- Violation 1: cross-references in type param bounds ---
            if trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
            {
                check_pep695_bound_cross_references(line, line_number, source, path, diagnostics);
            }

            // --- Violation 3: method re-defines class type param ---
            if trimmed.starts_with("class ") {
                check_method_redefines_class_type_param(
                    &lines,
                    line_number,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 2a: module-level use of PEP 695 type param ---
            if leading_indent(line) == 0 && !all_pep695_params.is_empty() {
                check_module_level_type_param_use(
                    line,
                    line_number,
                    &all_pep695_params,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 2b: decorator uses the decorated class's type param ---
            if trimmed.starts_with('@') && leading_indent(line) == 0 {
                check_decorator_uses_class_type_param(
                    &lines,
                    line_number,
                    source,
                    path,
                    diagnostics,
                );
            }
        }
    }
}
