//! BSK-E0130: `TypeVar` scoping violation.
//!
//! Detects uses of `TypeVar` instances outside their valid scope:
//!
//! 1. A nested class inside a generic class using the outer class's `TypeVar`
//!    in its base classes or body (the outer class's type params don't cover
//!    the inner class scope).
//! 2. A class nested inside a generic function re-using the function's `TypeVar`
//!    in `Generic[...]`.
//! 3. A `TypeVar` used in a module-level expression (subscript call like `list[T]()`).
//! 4. A method call on a generic class instance where the argument type does not
//!    match the substituted `TypeVar` type (e.g., `a: MyClass[int]`, calling
//!    `a.meth('str')` when `meth` expects `T` which is bound to `int`).
//!
//! Per PEP 484: "A generic class nested in another generic class cannot use
//! the same type variables."

use std::collections::HashMap;
use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0130",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0130",
};

/// Emits BSK-E0130 for `TypeVar` scoping violations.
pub(crate) struct TypeVarScopeViolation;

/// Represents a scope (class or function) with its indentation level and bound `TypeVars`.
struct ScopeInfo {
    /// Indentation column of the `class`/`def` keyword.
    indent: usize,
    /// `TypeVar` names bound by this scope (from `Generic[...]` params or function annotations).
    bound_typevars: HashSet<String>,
    /// Whether this is a class scope (vs function scope).
    is_class: bool,
}

/// Check if `name` appears as a whole identifier in `text` (not as part of a longer name).
fn contains_typevar_reference(text: &str, typevar_name: &str) -> bool {
    let needle = typevar_name.as_bytes();
    let haystack = text.as_bytes();
    let needle_len = needle.len();

    if needle_len > haystack.len() {
        return false;
    }

    haystack
        .windows(needle_len)
        .enumerate()
        .any(|(idx, window)| {
            if window != needle {
                return false;
            }
            let before_ok = idx == 0
                || (!haystack[idx - 1].is_ascii_alphanumeric() && haystack[idx - 1] != b'_');
            let after_ok = idx + needle_len >= haystack.len()
                || (!haystack[idx + needle_len].is_ascii_alphanumeric()
                    && haystack[idx + needle_len] != b'_');
            before_ok && after_ok
        })
}

/// Extract `TypeVar` names from a `Generic[T, S, ...]` or similar parameterized base.
fn extract_typevars_from_generic_base(line: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    if let Some(start) = line.find("Generic[") {
        let after = &line[start + 8..];
        if let Some(end) = after.find(']') {
            let params = &after[..end];
            for param in params.split(',') {
                let trimmed = param.trim();
                if !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    result.insert(trimmed.to_owned());
                }
            }
        }
    }
    result
}

/// Extract `TypeVar` names referenced in function parameter annotations and return type.
fn extract_typevars_from_function_sig(
    line: &str,
    all_typevars: &HashSet<String>,
) -> HashSet<String> {
    let mut result = HashSet::new();
    for typevar_name in all_typevars {
        if contains_typevar_reference(line, typevar_name) {
            result.insert(typevar_name.clone());
        }
    }
    result
}

/// Compute the leading whitespace count of a line.
fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Find the byte offset of a given 1-based line number in source text.
#[allow(clippy::cast_possible_truncation)]
fn line_to_byte_offset(source: &str, target_line: usize) -> u32 {
    let mut current_line = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current_line == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current_line += 1;
        }
    }
    source.len() as u32
}

/// Build a span covering the trimmed content of the given 1-based line.
#[allow(clippy::cast_possible_truncation)]
fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_to_byte_offset(source, line_number) as usize;
    let line_text = source[start..].lines().next().unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

/// A generic class definition discovered from source text.
struct GenericClassDef {
    /// The class name.
    name: String,
    /// `TypeVar` names in `Generic[T, S, ...]` order.
    typevar_params: Vec<String>,
    /// Methods: name -> list of (`param_name``annotation_text`xt) pairs (excluding `self`).
    methods: HashMap<String, Vec<(String, String)>>,
}

/// A module-level variable annotated with a concrete generic type.
struct GenericInstance {
    /// The variable name (e.g. `a`).
    var_name: String,
    /// The class name (e.g. `MyClass`).
    class_name: String,
    /// The concrete type args in order (e.g. `["int"]`).
    type_args: Vec<String>,
}

/// Extract the `TypeVar` names from a `Generic[T, S]` base expression.
fn extract_typevar_params_from_generic(source_line: &str) -> Vec<String> {
    let Some(start) = source_line.find("Generic[") else {
        return Vec::new();
    };
    let after = &source_line[start + 8..];
    let Some(end) = after.find(']') else {
        return Vec::new();
    };
    after[..end]
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a subscript annotation like `MyClass[int]` into `("MyClass", ["int"])`.
fn parse_generic_annotation(ann: &str) -> Option<(String, Vec<String>)> {
    let bracket_pos = ann.find('[')?;
    let class_name = ann[..bracket_pos].trim().to_owned();
    if class_name.is_empty() {
        return None;
    }
    let inner = ann.get(bracket_pos + 1..ann.rfind(']')?)?;
    let type_args = split_top_level_type_args(inner);
    if type_args.is_empty() {
        return None;
    }
    Some((class_name, type_args))
}

/// Split comma-separated type args at the top level of brackets.
fn split_top_level_type_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(inner[start..idx].trim().to_owned());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        args.push(last.to_owned());
    }
    args
}

/// Infer a simple Python type name from a literal expression.
fn infer_literal_type(expr: &str) -> Option<&'static str> {
    let expr = expr.trim();
    if expr == "True" || expr == "False" {
        return Some("bool");
    }
    if expr == "None" {
        return Some("None");
    }
    // Integer literal (possibly negative)
    if expr.chars().all(|c| c.is_ascii_digit())
        || (expr.starts_with('-')
            && expr.len() > 1
            && expr[1..].chars().all(|c| c.is_ascii_digit()))
    {
        return Some("int");
    }
    // Float literal
    if expr.contains('.')
        && expr
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return Some("float");
    }
    // String literal
    if (expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2)
        || (expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2)
    {
        return Some("str");
    }
    // Bytes literal
    if (expr.starts_with("b\"") && expr.ends_with('"'))
        || (expr.starts_with("b'") && expr.ends_with('\''))
    {
        return Some("bytes");
    }
    None
}

/// Check if two type names are compatible.
fn generic_types_compatible(actual: &str, expected: &str) -> bool {
    if expected == "Any" || actual == "Any" || expected == "object" {
        return true;
    }
    if actual == expected {
        return true;
    }
    // bool is subtype of int
    if expected == "int" && actual == "bool" {
        return true;
    }
    // int is subtype of float
    if expected == "float" && (actual == "int" || actual == "bool") {
        return true;
    }
    false
}

/// Scan source text to collect generic class definitions.
#[allow(clippy::too_many_lines)]
fn collect_generic_classes(source: &str) -> Vec<GenericClassDef> {
    let lines: Vec<&str> = source.lines().collect();
    let mut classes: Vec<GenericClassDef> = Vec::new();

    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        // Detect class definitions with Generic[...] base.
        if trimmed.starts_with("class ") && trimmed.contains("Generic[") {
            // Extract class name.
            let after_class = &trimmed[6..];
            let class_name = after_class
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .map_or(after_class, |pos| &after_class[..pos])
                .to_owned();

            // Extract TypeVar params from Generic[...].
            let typevar_params = extract_typevar_params_from_generic(trimmed);

            // Determine class body indentation (next non-empty, non-comment line).
            let class_indent = line.len() - line.trim_start().len();

            // Scan the class body for method definitions.
            let mut methods: HashMap<String, Vec<(String, String)>> = HashMap::new();
            let mut body_idx = idx + 1;
            while body_idx < lines.len() {
                let body_line = lines[body_idx];
                let body_trimmed = body_line.trim();

                if body_trimmed.is_empty() || body_trimmed.starts_with('#') {
                    body_idx += 1;
                    continue;
                }

                let body_indent = body_line.len() - body_line.trim_start().len();

                // If we hit something at or before class indent that's not empty, stop.
                if body_indent <= class_indent && !body_trimmed.is_empty() {
                    break;
                }

                // Only look at direct methods (one indent level deeper).
                if body_indent == class_indent + 4 && body_trimmed.starts_with("def ") {
                    // Parse the method signature.
                    let after_def = &body_trimmed[4..];
                    if let Some(paren_pos) = after_def.find('(') {
                        let method_name = after_def[..paren_pos].trim().to_owned();
                        // Extract params from the full signature text.
                        // Check that the closing paren is on this line (single-line defs only).
                        let mut depth = 0i32;
                        let mut found_close = false;
                        for ch in body_trimmed.chars() {
                            match ch {
                                '(' => depth += 1,
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        found_close = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        // If not found on this line, skip (multi-line not supported).
                        if !found_close {
                            body_idx += 1;
                            continue;
                        }

                        // Extract params from within parens.
                        if let Some(open) = body_trimmed.find('(') {
                            if let Some(close) = body_trimmed.rfind(')') {
                                let params_text = &body_trimmed[open + 1..close];
                                let params = parse_method_params(params_text);
                                methods.insert(method_name, params);
                            }
                        }
                    }
                }

                body_idx += 1;
            }

            classes.push(GenericClassDef {
                name: class_name,
                typevar_params,
                methods,
            });
        }

        idx += 1;
    }

    classes
}

/// Parse method parameters text (inside parens), returning `(param_name, annotation)` pairs.
/// Skips `self`.
fn parse_method_params(params_text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut param_parts: Vec<&str> = Vec::new();

    for (idx, ch) in params_text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                param_parts.push(params_text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    param_parts.push(params_text[start..].trim());

    let mut first = true;
    for param in param_parts {
        if param.is_empty() {
            continue;
        }
        // Skip `self` and `cls`.
        if first {
            let param_name = param.split(':').next().unwrap_or(param).trim();
            if param_name == "self" || param_name == "cls" {
                first = false;
                continue;
            }
        }
        first = false;

        // Parse `name: annotation` or just `name`.
        if let Some(colon_pos) = param.find(':') {
            let param_name = param[..colon_pos].trim().to_owned();
            // Strip defaults: find `=` at depth 0 in annotation.
            let ann_raw = &param[colon_pos + 1..];
            let annotation = ann_raw
                .split('=')
                .next()
                .unwrap_or(ann_raw)
                .trim()
                .to_owned();
            result.push((param_name, annotation));
        }
    }
    result
}

/// Collect module-level variables annotated with a concrete generic type.
fn collect_generic_instances(source: &str) -> Vec<GenericInstance> {
    let mut instances = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        // Only look at lines at module level (no leading indent for simplicity)
        // and annotated assignments: `name: GenericClass[Type] = ...`
        if line.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        // Must contain `:` and `[` (annotation with generic type).
        if !trimmed.contains(':') || !trimmed.contains('[') {
            continue;
        }
        // Skip class/def lines.
        if trimmed.starts_with("class ") || trimmed.starts_with("def ") {
            continue;
        }
        // Skip comment lines.
        if trimmed.starts_with('#') {
            continue;
        }
        // Find `:` for annotation.
        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let var_name = trimmed[..colon_pos].trim();
        // Variable name must be a simple identifier.
        if var_name.is_empty() || !var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        // Extract annotation (up to `=` or end of line, stripping comments).
        let after_colon = &trimmed[colon_pos + 1..];
        let ann_raw = after_colon.split('=').next().unwrap_or(after_colon).trim();
        let ann_text = ann_raw.split('#').next().unwrap_or(ann_raw).trim();

        if let Some((class_name, type_args)) = parse_generic_annotation(ann_text) {
            instances.push(GenericInstance {
                var_name: var_name.to_owned(),
                class_name,
                type_args,
            });
        }
    }

    instances
}

/// Check module-level method calls on generic class instances for type mismatches.
#[allow(clippy::too_many_lines)]
fn check_generic_instance_method_calls(
    source: &str,
    all_typevars: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    let generic_classes = collect_generic_classes(source);
    if generic_classes.is_empty() {
        return;
    }

    let instances = collect_generic_instances(source);
    if instances.is_empty() {
        return;
    }

    // Build a map: var_name -> (class_name, typevar_substitution_map)
    let mut var_substitutions: HashMap<String, (&GenericClassDef, HashMap<String, String>)> =
        HashMap::new();

    for instance in &instances {
        if let Some(class_def) = generic_classes
            .iter()
            .find(|c| c.name == instance.class_name)
        {
            let subst: HashMap<String, String> = class_def
                .typevar_params
                .iter()
                .zip(instance.type_args.iter())
                .map(|(tv, ty)| (tv.clone(), ty.clone()))
                .collect();
            var_substitutions.insert(instance.var_name.clone(), (class_def, subst));
        }
    }

    if var_substitutions.is_empty() {
        return;
    }

    // Scan source lines for method calls like `var_name.method_name(args...)`.
    for (line_idx, line) in source.lines().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();

        // Skip empty lines, comments, class/def defs.
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("class ")
            || trimmed.starts_with("def ")
        {
            continue;
        }

        // Only check module-level calls (no indentation).
        if line.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }

        // Look for `var_name.method_name(args)` patterns.
        for (var_name, (class_def, subst)) in &var_substitutions {
            let dot_pattern = format!("{var_name}.");
            let Some(dot_pos) = trimmed.find(dot_pattern.as_str()) else {
                continue;
            };
            let after_dot = &trimmed[dot_pos + dot_pattern.len()..];

            // Extract method name.
            let paren_pos = after_dot.find('(');
            let Some(paren_pos) = paren_pos else {
                continue;
            };
            let method_name = after_dot[..paren_pos].trim();

            // Look up this method in the class def.
            let Some(method_params) = class_def.methods.get(method_name) else {
                continue;
            };

            // Extract the args text from within the parens.
            let after_paren = &after_dot[paren_pos + 1..];
            let close_paren = find_matching_close(after_paren);
            let args_text = &after_paren[..close_paren];

            // Split args at top-level commas.
            let args = split_top_level_type_args(args_text);

            // Check each parameter.
            for (param_idx, (param_name, param_ann)) in method_params.iter().enumerate() {
                // Only check if the annotation is a TypeVar that's in the class's generic params.
                let typevar_name = param_ann.trim();
                if !all_typevars.contains(typevar_name) {
                    continue;
                }
                // Check if this TypeVar is substituted in this instance.
                let Some(expected_type) = subst.get(typevar_name) else {
                    continue;
                };

                // Get the corresponding argument.
                let Some(arg_raw) = args.get(param_idx) else {
                    continue;
                };
                let arg_trimmed = arg_raw.trim();

                // Infer the type of the argument literal.
                let Some(actual_type) = infer_literal_type(arg_trimmed) else {
                    // Cannot infer — skip (conservative, no false positives).
                    continue;
                };

                if !generic_types_compatible(actual_type, expected_type) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Argument `{arg_trimmed}` of type `{actual_type}` is not compatible \
                             with parameter `{param_name}: {expected_type}` \
                             (TypeVar `{typevar_name}` is bound to `{expected_type}` \
                             for instance `{var_name}: {}[{}]`)",
                            class_def.name,
                            class_def
                                .typevar_params
                                .iter()
                                .zip(subst.values())
                                .map(|(tv, ty)| format!("{tv}={ty}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        span: span_for_line(source, line_number),
                        path: path.to_owned(),
                        help: Some(format!(
                            "Parameter `{param_name}` expects `{expected_type}` \
                             because `{typevar_name}` is bound to `{expected_type}` \
                             for this instance"
                        )),
                        note: Some(
                            "PEP 484: type variables in methods of generic classes \
                             are bound to the class's type arguments"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Find the position of the matching close paren/bracket in a string that starts
/// after an opening delimiter.
fn find_matching_close(text: &str) -> usize {
    let mut depth = 0i32;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return idx;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    text.len()
}

impl Rule for TypeVarScopeViolation {
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let all_typevars: HashSet<String> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.clone())
            .collect();

        if all_typevars.is_empty() {
            return;
        }

        let lines: Vec<&str> = module.source.lines().collect();

        // Track scope stack: each entry has (indent, bound_typevars, is_class).
        let mut scope_stack: Vec<ScopeInfo> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // Skip empty lines, comments, and pure string lines.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = leading_indent(line);

            // Pop scopes that are no longer active (indentation decreased).
            while let Some(top) = scope_stack.last() {
                if indent <= top.indent {
                    scope_stack.pop();
                } else {
                    break;
                }
            }

            // Detect class definitions.
            if trimmed.starts_with("class ") {
                let bound_tvs = extract_typevars_from_generic_base(trimmed);

                // Check: does this class's base reference a TypeVar from an outer scope?
                let outer_bound: HashSet<String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter().cloned())
                    .collect();

                if !outer_bound.is_empty() {
                    // Check if any outer TypeVar is used in the base classes.
                    // Extract the base class portion: everything between `(` and `)`.
                    if let Some(paren_start) = trimmed.find('(') {
                        if let Some(paren_end) = trimmed.rfind(')') {
                            let bases_text = &trimmed[paren_start + 1..paren_end];
                            for typevar_name in &outer_bound {
                                if contains_typevar_reference(bases_text, typevar_name) {
                                    diagnostics.push(Diagnostic {
                                        code: CODE.clone(),
                                        severity: Severity::Error,
                                        message: format!(
                                            "TypeVar `{typevar_name}` from outer scope \
                                             cannot be used in nested class definition"
                                        ),
                                        span: span_for_line(&module.source, line_number),
                                        path: module.path.clone(),
                                        help: Some(
                                            "Use a different TypeVar for the inner class, \
                                             or restructure to avoid nesting"
                                                .to_owned(),
                                        ),
                                        note: Some(
                                            "PEP 484: the scope of type variables of the \
                                             outer class doesn't cover the inner one"
                                                .to_owned(),
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }

                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: true,
                });
                continue;
            }

            // Detect function definitions.
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let bound_tvs = extract_typevars_from_function_sig(trimmed, &all_typevars);
                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: false,
                });
                continue;
            }

            // For lines inside a nested class body (inner class of a generic class),
            // check if annotations reference outer class TypeVars.
            // This only applies when there are at least 2 class scopes on the stack
            // (outer generic class + inner class).
            {
                let class_scopes: Vec<&ScopeInfo> =
                    scope_stack.iter().filter(|scope| scope.is_class).collect();

                if class_scopes.len() >= 2 {
                    // Collect TypeVars from all outer class scopes (all but the last).
                    let outer_class_tvs: HashSet<String> = class_scopes[..class_scopes.len() - 1]
                        .iter()
                        .flat_map(|scope| scope.bound_typevars.iter().cloned())
                        .collect();

                    let innermost_tvs = &class_scopes
                        .last()
                        .map_or_else(HashSet::new, |scope| scope.bound_typevars.clone());

                    let forbidden_tvs: HashSet<&String> = outer_class_tvs
                        .iter()
                        .filter(|tv| !innermost_tvs.contains(*tv))
                        .collect();

                    if !forbidden_tvs.is_empty() && trimmed.contains(':') {
                        // This is an annotation line — check for forbidden TypeVar refs.
                        let annotation_part =
                            trimmed.split_once(':').map_or(trimmed, |(_, rhs)| rhs);
                        for typevar_name in &forbidden_tvs {
                            if contains_typevar_reference(annotation_part, typevar_name) {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` from outer class \
                                         cannot be used in nested class body"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "Use a different TypeVar for the inner class".to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: the scope of type variables of the \
                                         outer class doesn't cover the inner one"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            // Check for unbound TypeVar usage in function bodies and class
            // attribute annotations. A TypeVar is "unbound" if no enclosing
            // scope binds it (via Generic[...] or function signature).
            if !scope_stack.is_empty() && trimmed.contains(':') {
                // Collect all TypeVars bound by any enclosing scope.
                let all_bound: HashSet<&String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter())
                    .collect();

                let annotation_part = trimmed.split_once(':').map_or(trimmed, |(_, rhs)| rhs);
                let before_comment = annotation_part
                    .split_once('#')
                    .map_or(annotation_part, |(code, _)| code);

                for typevar_name in &all_typevars {
                    if !all_bound.contains(typevar_name)
                        && contains_typevar_reference(before_comment, typevar_name)
                    {
                        // Skip if this is a function def line (already handles its own
                        // TypeVars via scope creation).
                        let is_def_line =
                            trimmed.starts_with("def ") || trimmed.starts_with("async def ");
                        if !is_def_line {
                            diagnostics.push(Diagnostic {
                                code: CODE.clone(),
                                severity: Severity::Error,
                                message: format!(
                                    "TypeVar `{typevar_name}` is not bound in \
                                     this scope"
                                ),
                                span: span_for_line(&module.source, line_number),
                                path: module.path.clone(),
                                help: Some(
                                    "TypeVars can only be used where they are \
                                     bound by a Generic[...] base or function signature"
                                        .to_owned(),
                                ),
                                note: Some(
                                    "PEP 484: unbound type variables should not \
                                     appear in function or class bodies"
                                        .to_owned(),
                                ),
                            });
                        }
                    }
                }
            }

            // TypeAlias inside a class body: class TypeVars are not in scope
            // for type alias definitions. `alias: TypeAlias = list[T]` is invalid
            // when T comes from the enclosing class's Generic[T].
            {
                let class_scopes: Vec<&ScopeInfo> =
                    scope_stack.iter().filter(|scope| scope.is_class).collect();

                if class_scopes.len() == 1 && trimmed.contains("TypeAlias") {
                    let enclosing_tvs = &class_scopes[0].bound_typevars;
                    if !enclosing_tvs.is_empty() {
                        // Check the RHS of the TypeAlias assignment for TypeVar refs.
                        let rhs_part = trimmed.split_once('=').map_or("", |(_, rhs)| rhs);
                        for typevar_name in enclosing_tvs {
                            if contains_typevar_reference(rhs_part, typevar_name) {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` from enclosing class \
                                         is not accessible in a TypeAlias definition"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "Type aliases in class bodies cannot reference \
                                         the class's type parameters"
                                            .to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: TypeAlias creates its own scope and \
                                         cannot capture class-level TypeVars"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            // Module-level (indent == 0, no enclosing scope): check for TypeVar
            // subscript expressions like `list[T]()`.
            if indent == 0 && scope_stack.is_empty() {
                // Skip class/def definitions, imports, assignments with annotations
                // (those are already handled by other checks or are valid).
                let dominated_by_other = trimmed.starts_with("class ")
                    || trimmed.starts_with("def ")
                    || trimmed.starts_with("import ")
                    || trimmed.starts_with("from ")
                    || trimmed.starts_with('@');

                if !dominated_by_other {
                    let before_comment = trimmed.split_once('#').map_or(trimmed, |(code, _)| code);
                    for typevar_name in &all_typevars {
                        // Check for TypeVar in annotations (x: T, x: list[T] = ...)
                        // and in expressions (list[T]()).
                        if contains_typevar_reference(before_comment, typevar_name) {
                            // Skip lines that are TypeVar definitions themselves
                            // (e.g. `T = TypeVar('T')`)
                            let is_typevar_def = before_comment.contains("TypeVar");
                            if !is_typevar_def {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` is not bound in \
                                         this scope"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "TypeVars can only be used inside generic \
                                         functions or classes that bind them"
                                            .to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: unbound type variables should not \
                                         appear at module scope"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Check generic class instance method calls for TypeVar substitution violations.
        check_generic_instance_method_calls(
            &module.source,
            &all_typevars,
            diagnostics,
            &module.path,
        );
    }
}
