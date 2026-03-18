//! The three PEP 695 scoping violation checks for BSK-E0149.

use crate::diagnostic::{Diagnostic, Severity};

use super::{
    helpers::{
        contains_name, extract_bound, extract_pep695_type_params, leading_indent, span_for_line,
    },
    CODE,
};

// ---------------------------------------------------------------------------
// Shared: collect PEP 695 type params across the file
// ---------------------------------------------------------------------------

/// Collect all PEP 695 type parameter names defined in the file.
/// Returns a list of (`param_name`, `defining_line_number`).
pub(super) fn collect_pep695_type_params(source: &str) -> Vec<(String, usize)> {
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

// ---------------------------------------------------------------------------
// Violation 1: forward / backward references in PEP 695 type param bounds
// ---------------------------------------------------------------------------

/// Check whether a PEP 695 type parameter list has bound violations where a
/// param's bound references another param in the same list.
///
/// Per PEP 695: referencing *any* other type param in the same list from a
/// bound is an error (whether earlier or later).
pub(super) fn check_pep695_bound_cross_references(
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
                let Some(param_name) = all_names.get(idx) else {
                    continue;
                };
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
pub(super) fn check_decorator_uses_class_type_param(
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

    let Some(target_trimmed) = lines.get(target_lno - 1).map(|l| l.trim()) else {
        return;
    };

    // Collect type params of the target definition.
    let Some(target_params) = extract_pep695_type_params(target_trimmed) else {
        return;
    };
    if target_params.is_empty() {
        return;
    }

    // Check the decorator line for references to those type params.
    let Some(decorator_text) = lines.get(decorator_line - 1) else {
        return;
    };
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
pub(super) fn check_module_level_type_param_use(
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
// Violation 4: PEP 695 `type` statement uses old-style TypeVar
// ---------------------------------------------------------------------------

/// Check whether a PEP 695 `type` statement's RHS references an old-style
/// `TypeVar` (defined via `TypeVar()` call, not PEP 695 brackets).
///
/// Per PEP 695: "Type aliases defined using the `type` statement must not
/// reference `TypeVar`, `ParamSpec`, or `TypeVarTuple` objects that are
/// created outside of the `type` statement's scope."
pub(super) fn check_type_stmt_uses_old_typevar(
    line: &str,
    line_number: usize,
    old_typevar_names: &[&str],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed = line.trim();
    if !trimmed.starts_with("type ") {
        return;
    }

    // Get the RHS (after `=`)
    let Some(eq_pos) = trimmed.find('=') else {
        return;
    };
    let rhs = &trimmed[eq_pos + 1..];
    // Strip comment
    let rhs = rhs.split_once('#').map_or(rhs, |(code, _)| code).trim();

    // Collect the new-style type param names between `type Name[` and `]`
    // by looking at the text BEFORE the `=`.
    let before_eq = &trimmed[..eq_pos];
    let new_params: Vec<String> = if let Some(bracket_pos) = before_eq.find('[') {
        // Extract text between `[` and `]` before `=`
        let after = &before_eq[bracket_pos + 1..];
        if let Some(close) = after.rfind(']') {
            after[..close]
                .split(',')
                .map(|p| {
                    p.trim()
                        .split(':')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_start_matches('*')
                        .to_owned()
                })
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    for old_tv_name in old_typevar_names {
        // Skip if this old-style TypeVar name is also a new-style param
        // on this same statement
        if new_params.iter().any(|p| p == old_tv_name) {
            continue;
        }
        if contains_name(rhs, old_tv_name) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!("PEP 695 `type` statement uses old-style TypeVar `{old_tv_name}`"),
                span: span_for_line(source, line_number),
                path: path.to_owned(),
                help: Some(format!(
                    "Use PEP 695 type parameter syntax instead: \
                     `type Alias[{old_tv_name}] = ...`"
                )),
                note: Some(
                    "PEP 695: type aliases defined with `type` must not reference \
                     TypeVars created outside the statement's scope"
                        .to_owned(),
                ),
            });
            // One diagnostic per type statement is enough
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 5: `type` statement inside function body
// ---------------------------------------------------------------------------

/// Check whether a `type` statement appears inside a function body.
///
/// PEP 695 type aliases defined with `type` are only valid at module or
/// class scope, not inside functions.
pub(super) fn check_type_stmt_in_function(
    line: &str,
    line_number: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed = line.trim();
    if !trimmed.starts_with("type ") {
        return;
    }

    // If the line has indentation, it might be inside a function.
    // We need to check if any enclosing scope is a function (not a class).
    let indent = leading_indent(line);
    if indent == 0 {
        return; // Module level — OK
    }

    // Walk backwards to find the enclosing def/class
    let lines: Vec<&str> = source.lines().collect();
    for scan_idx in (0..line_number.saturating_sub(1)).rev() {
        let scan_line = lines.get(scan_idx).copied().unwrap_or("");
        let scan_indent = leading_indent(scan_line);
        let scan_trimmed = scan_line.trim();

        if scan_indent < indent && !scan_trimmed.is_empty() {
            if scan_trimmed.starts_with("def ") || scan_trimmed.starts_with("async def ") {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: "PEP 695 `type` statement is not allowed inside a function body"
                        .to_owned(),
                    span: span_for_line(source, line_number),
                    path: path.to_owned(),
                    help: Some("Move the type alias to module or class scope".to_owned()),
                    note: Some(
                        "PEP 695: type aliases defined with `type` are only valid at \
                         module or class scope"
                            .to_owned(),
                    ),
                });
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 6: circular type alias definition
// ---------------------------------------------------------------------------

/// Check whether a PEP 695 `type` statement has a circular self-reference.
///
/// A `type` statement is circular if the alias name appears in its own RHS
/// in a way that doesn't go through a type parameter (legitimate recursion
/// goes through `TypeAlias[T]` where `T` is a param of the same statement).
pub(super) fn check_type_stmt_circular(
    line: &str,
    line_number: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed = line.trim();
    if !trimmed.starts_with("type ") {
        return;
    }

    // Extract alias name: `type Name[...] = ...` or `type Name = ...`
    let after_type = &trimmed["type ".len()..];
    let name_end = after_type
        .find(['[', '=', ' ', ':'])
        .unwrap_or(after_type.len());
    let alias_name = after_type[..name_end].trim();
    if alias_name.is_empty() {
        return;
    }

    // Get the RHS
    let Some(eq_pos) = trimmed.find('=') else {
        return;
    };
    let rhs = &trimmed[eq_pos + 1..];
    let rhs = rhs.split_once('#').map_or(rhs, |(code, _)| code).trim();

    // Check if the alias name appears in its own RHS
    if !contains_name(rhs, alias_name) {
        return;
    }

    // Collect type params of this `type` statement
    let before_eq = &trimmed[..eq_pos];
    let has_type_params = before_eq.contains('[');

    if !has_type_params {
        // No type params — any self-reference is circular
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!("Circular type alias definition: `{alias_name}` references itself"),
            span: span_for_line(source, line_number),
            path: path.to_owned(),
            help: Some("A type alias cannot reference itself without type parameters".to_owned()),
            note: None,
        });
        return;
    }

    // Has type params — check if the self-reference uses DIFFERENT type args
    // than the params (making it circular rather than recursive).
    // E.g. `type Foo[T] = T | Foo[str]` — Foo[str] uses concrete `str`,
    // not the type param `T`, so this is a circular definition.
    // But `type Foo[T] = T | list[Foo[T]]` is legitimate recursion.

    // Find the self-reference subscript in the RHS
    let param_text = if let Some(bracket_pos) = before_eq.find('[') {
        let after = &before_eq[bracket_pos + 1..];
        after.rfind(']').map_or("", |close| &after[..close])
    } else {
        ""
    };
    let params: Vec<&str> = param_text
        .split(',')
        .map(|p| {
            p.trim()
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches('*')
        })
        .filter(|s| !s.is_empty())
        .collect();

    // Find self-reference in RHS and check its args
    if let Some(self_ref_pos) = rhs.find(alias_name) {
        let after_name = &rhs[self_ref_pos + alias_name.len()..];
        if let Some(bracket_content) = after_name.strip_prefix('[') {
            if let Some(close) = bracket_content.find(']') {
                let args: Vec<&str> = bracket_content[..close].split(',').map(str::trim).collect();
                // If the args don't exactly match the params, it's circular
                let is_identity = args.len() == params.len()
                    && args
                        .iter()
                        .zip(params.iter())
                        .all(|(arg, param)| arg == param);
                if !is_identity {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Circular type alias definition: `{alias_name}` references \
                             itself with different type arguments"
                        ),
                        span: span_for_line(source, line_number),
                        path: path.to_owned(),
                        help: Some(
                            "Recursive type aliases must reference themselves with the \
                             same type parameters"
                                .to_owned(),
                        ),
                        note: None,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 3: method re-defines class type param with its own [T]
// ---------------------------------------------------------------------------

/// Inside a generic class body (`class Foo[T]`), check if any method
/// defines its own type parameter list that re-uses the class's type param
/// names.  Per PEP 695, this is a scoping violation.
pub(super) fn check_method_redefines_class_type_param(
    lines: &[&str],
    class_line: usize,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(class_line_text) = lines.get(class_line - 1) else {
        return;
    };
    let class_trimmed = class_line_text.trim();
    let Some(class_params) = extract_pep695_type_params(class_trimmed) else {
        return;
    };
    if class_params.is_empty() {
        return;
    }

    let class_indent = leading_indent(class_line_text);
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
