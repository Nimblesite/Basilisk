//! BSK-E0128: ```TypeVar``` default referential violations.
//!
//! PEP 696 defines rules for when a `TypeVar` default references another
//! `TypeVar`:
//!
//! 1. **Ordering**: The referenced `TypeVar` must appear *before* the referencing
//!    `TypeVar` in `Generic[...]`.
//! 2. **Scope**: A `TypeVar` default must not reference `TypeVar`ar from an outer
//!    class scope.
//! 3. **Bound/constraint compatibility**: When `TypeVar` `T2` defaults to
//!    `TypeVar` `T1`, `T1`'s bound must be a subtype of `T2`'s bound, and
//!    `T2`'s constraints (if any) must be a superset of `T1`'s constraints.
//!
//! ```python
//! from typing import TypeVar, Generic
//!
//! S1 = TypeVar("S1")
//! S2 = TypeVar("S2", default=S1)
//!
//! Start2T = TypeVar("Start2T", default="StopT")
//! Stop2T = TypeVar("Stop2T", default=int)
//! class slice2(Generic[Start2T, Stop2T]): ...   # E: bad ordering
//!
//! class Foo3(Generic[S1]):
//!     class Bar2(Generic[S2]): ...              # E: outer scope
//!
//! Y1 = TypeVar("Y1", bound=int)
//! Invalid2 = TypeVar("Invalid2", float, str, default=Y1)  # E
//! ```

#![allow(clippy::too_many_lines)]

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0128",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0128",
};

/// Information about a `TypeVar` extracted from source text.
struct TypeVarInfo {
    /// Name of the `TypeVar` (LHS of assignment).
    name: String,
    /// Name of the `TypeVar` referenced in `default=`, if any.
    default_typevar_name: Option<String>,
    /// The bound type name, if `bound=` is present.
    bound_name: Option<String>,
    /// Constraint type names (positional args after the name string).
    constraint_names: Vec<String>,
}

/// Parse `TypeVar` definitions from source text to extract default value names,
/// bound names, and constraint names that are not available in the resolver's
/// `TypeVarCallInfo`.
fn parse_typevar_info_from_source(source: &str, typevar_names: &HashSet<&str>) -> Vec<TypeVarInfo> {
    let mut results = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        // Look for patterns like: Name = TypeVar("Name", ..., default=X)
        let Some(eq_pos) = trimmed.find('=') else {
            continue;
        };

        // Ensure it's not == or !=
        if trimmed.as_bytes().get(eq_pos + 1) == Some(&b'=') {
            continue;
        }
        if eq_pos > 0 && trimmed.as_bytes().get(eq_pos - 1) == Some(&b'!') {
            continue;
        }

        let lhs = trimmed[..eq_pos].trim();
        let rhs = trimmed[eq_pos + 1..].trim();

        // Must be a simple identifier on LHS
        if !lhs.chars().all(|c| c.is_alphanumeric() || c == '_') || lhs.is_empty() {
            continue;
        }

        // RHS must start with TypeVar(
        if !rhs.starts_with("TypeVar(") {
            continue;
        }

        let inner = match rhs.strip_prefix("TypeVar(") {
            Some(rest) => {
                // Find matching closing paren
                let mut depth = 1i32;
                let mut end = 0;
                for (idx, ch) in rest.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = idx;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                &rest[..end]
            }
            None => continue,
        };

        let mut info = TypeVarInfo {
            name: lhs.to_owned(),
            default_typevar_name: None,
            bound_name: None,
            constraint_names: Vec::new(),
        };

        // Parse args: skip the first string arg (name), collect constraints and kwargs
        let args = split_top_level_args(inner);

        let mut past_name = false;
        for arg in &args {
            let arg = arg.trim();

            if !past_name {
                // First arg is the name string
                past_name = true;
                continue;
            }

            if let Some(val) = arg.strip_prefix("default=") {
                let val = val.trim().trim_matches('"');
                // Only record the default if it references a known TypeVar
                if typevar_names.contains(val) {
                    info.default_typevar_name = Some(val.to_owned());
                }
            } else if let Some(val) = arg.strip_prefix("bound=") {
                info.bound_name = Some(val.trim().to_owned());
            } else if !arg.contains('=') {
                // Positional arg = constraint
                info.constraint_names.push(arg.to_owned());
            }
        }

        results.push(info);
    }

    results
}

/// Split a string by commas at the top level (not inside brackets or parens).
fn split_top_level_args(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in text.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                result.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        result.push(current);
    }
    result
}

/// Built-in numeric type hierarchy for bound/constraint compatibility checks.
fn is_numeric_subtype(sub: &str, super_type: &str) -> bool {
    match (sub, super_type) {
        (a, b) if a == b => true,
        ("bool", "int" | "float" | "complex")
        | ("int", "float" | "complex")
        | ("float", "complex") => true,
        _ => false,
    }
}

/// Emits BSK-E0128 for `TypeVar` default referential violations.
pub(crate) struct TypeVarDefaultReferential;

impl Rule for TypeVarDefaultReferential {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let typevar_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        let typevar_info_list = parse_typevar_info_from_source(&module.source, &typevar_names);

        // Build lookup from name to info
        let info_map: HashMap<&str, &TypeVarInfo> = typevar_info_list
            .iter()
            .map(|info| (info.name.as_str(), info))
            .collect();

        // Build lookup from name to resolver TypeVarCallInfo for spans
        let span_map: HashMap<&str, &basilisk_resolver::TypeVarCallInfo> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv))
            .collect();

        // Check 1: Forward references in Generic[...] params (ordering)
        check_ordering(module, &info_map, &typevar_names, diagnostics);

        // Check 2: Outer scope references
        check_outer_scope(module, &info_map, &typevar_names, diagnostics);

        // Check 3: Bound/constraint compatibility
        check_bound_constraint_compat(
            &typevar_info_list,
            &info_map,
            &span_map,
            &typevar_names,
            &module.path,
            diagnostics,
        );

        // Check 4: Subscripted generic class calls with literal arg mismatches
        check_subscripted_class_calls(module, &info_map, diagnostics);
    }
}

/// Check that `TypeVar` defaults don't referenc`TypeVars`rs that appear later
/// in the Generic[...] parameter list, or that are not in the list at all.
fn check_ordering(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        if class.generic_params.is_empty() {
            continue;
        }

        let param_names: Vec<&str> = class
            .generic_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();

        for (idx, param) in class.generic_params.iter().enumerate() {
            let Some(info) = info_map.get(param.name.as_str()) else {
                continue;
            };
            let Some(ref default_name) = info.default_typevar_name else {
                continue;
            };

            if !typevar_names.contains(default_name.as_str()) {
                continue;
            }

            let ref_pos = param_names.iter().position(|&n| n == default_name);

            let is_violation = match ref_pos {
                Some(pos) => pos > idx, // appears after this param
                None => true,           // not in this class's Generic at all
            };

            if is_violation {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "TypeVar `{}` has default `{}` which is not properly ordered \
                         in `Generic[...]` for `{}`",
                        param.name, default_name, class.name
                    ),
                    span: class.name_span,
                    path: module.path.clone(),
                    help: Some(
                        "The referenced TypeVar must appear before the TypeVar \
                         that defaults to it in the same Generic parameter list"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 696: a TypeVar default must reference a TypeVar that \
                         appears earlier in the same Generic parameter list"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Check that `TypeVar` defaults don't referenc`TypeVars`rs from an outer class scope.
///
/// Since the resolver doesn't track nested classes, we detect nested
/// `class ... (Generic[...])` patterns by scanning the source for indented
/// class definitions inside outer classes.
fn check_outer_scope(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Collect outer class TypeVar params
    let mut outer_class_params: HashSet<&str> = HashSet::new();
    for class in &module.classes {
        for param in &class.generic_params {
            outer_class_params.insert(param.name.as_str());
        }
    }

    // Scan source for nested class definitions with Generic[...]
    // Pattern: indented `class Name(Generic[...]):` inside a class body
    let lines: Vec<&str> = module.source.lines().collect();
    let mut inside_class = false;
    let mut class_indent = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Detect outer class start
        if trimmed.starts_with("class ") && indent == 0 {
            inside_class = true;
            class_indent = indent;
            continue;
        }

        // Detect nested class inside an outer class
        if inside_class && indent > class_indent && trimmed.starts_with("class ") {
            // Extract Generic params from nested class
            if let Some(generic_start) = trimmed.find("Generic[") {
                let after = &trimmed[generic_start + 8..];
                if let Some(bracket_end) = after.find(']') {
                    let params_str = &after[..bracket_end];
                    let nested_params: Vec<&str> = params_str.split(',').map(str::trim).collect();

                    for nested_param in &nested_params {
                        let Some(info) = info_map.get(nested_param) else {
                            continue;
                        };
                        let Some(ref default_name) = info.default_typevar_name else {
                            continue;
                        };

                        if !typevar_names.contains(default_name.as_str()) {
                            continue;
                        }

                        // Is the default referencing an outer class's param?
                        let in_nested = nested_params.contains(&default_name.as_str());
                        if !in_nested && outer_class_params.contains(default_name.as_str()) {
                            // Compute byte offset for this line
                            let byte_offset: u32 = u32::try_from(
                                lines[..line_idx].iter().map(|l| l.len() + 1).sum::<usize>(),
                            )
                            .unwrap_or(u32::MAX);
                            let line_len = u32::try_from(line.len()).unwrap_or(u32::MAX);

                            diagnostics.push(Diagnostic {
                                code: CODE.clone(),
                                severity: Severity::Error,
                                message: format!(
                                    "TypeVar `{nested_param}` has default `{default_name}` which references an \
                                     outer-scope TypeVar",
                                ),
                                span: basilisk_resolver::Span {
                                    start: byte_offset,
                                    end: byte_offset + line_len,
                                },
                                path: module.path.clone(),
                                help: Some(
                                    "TypeVar defaults cannot reference TypeVars from an \
                                     enclosing class scope"
                                        .to_owned(),
                                ),
                                note: Some(
                                    "PEP 696: using a type parameter from an outer scope \
                                     as a default is not supported"
                                        .to_owned(),
                                ),
                            });
                        }
                    }
                }
            }
        }

        // Reset inside_class when we return to top-level
        if inside_class && indent == 0 && !trimmed.is_empty() && !trimmed.starts_with('#') {
            inside_class = false;
        }
    }
}

/// Check that when `TypeVar` T2 defaults t`TypeVar`ar T1:
/// - T1's bound is a subtype of T2's bound (if T2 has a bound)
/// - T2's constraints are a superset of T1's constraints (if T2 has constraints)
fn check_bound_constraint_compat(
    typevar_info_list: &[TypeVarInfo],
    info_map: &HashMap<&str, &TypeVarInfo>,
    span_map: &HashMap<&str, &basilisk_resolver::TypeVarCallInfo>,
    typevar_names: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for info in typevar_info_list {
        let Some(ref default_name) = info.default_typevar_name else {
            continue;
        };

        // The default must reference a known TypeVar
        if !typevar_names.contains(default_name.as_str()) {
            continue;
        }

        let Some(ref_info) = info_map.get(default_name.as_str()) else {
            continue;
        };

        let Some(tv) = span_map.get(info.name.as_str()) else {
            continue;
        };

        // Check bound compatibility: ref_info's bound must be subtype of info's bound
        if let Some(ref info_bound) = info.bound_name {
            if let Some(ref ref_bound) = ref_info.bound_name {
                if !is_numeric_subtype(ref_bound, info_bound) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "TypeVar `{}` has default `{}` with bound `{}` which is not a \
                             subtype of `{}`'s bound `{}`",
                            info.name, default_name, ref_bound, info.name, info_bound
                        ),
                        span: tv.span,
                        path: path.to_owned(),
                        help: Some(
                            "The referenced TypeVar's bound must be a subtype of this TypeVar's \
                             bound"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 696: T1's bound must be a subtype of T2's bound when T2 \
                             defaults to T1"
                                .to_owned(),
                        ),
                    });
                }
            }
        }

        // Check constraint compatibility when info has constraints:
        // info's constraints must be a superset of ref_info's constraints.
        // Also check: if ref_info has a bound but info has constraints,
        // the bound must be compatible with the constraints.
        if !info.constraint_names.is_empty() {
            if let Some(ref ref_bound) = ref_info.bound_name {
                // ref has a bound, info has constraints — for TypeVar constraints,
                // the bound must exactly match one of the constraints (constraints
                // are an exact set, not a subtype hierarchy)
                let compatible = info.constraint_names.iter().any(|c| c == ref_bound);
                if !compatible {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "TypeVar `{}` has default `{}` with upper bound `{}` which is \
                             incompatible with constraints `{}`",
                            info.name,
                            default_name,
                            ref_bound,
                            info.constraint_names.join(", ")
                        ),
                        span: tv.span,
                        path: path.to_owned(),
                        help: Some(
                            "The referenced TypeVar's bound must be compatible with this \
                             TypeVar's constraints"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 696: the upper bound of the default TypeVar must be compatible \
                             with the constrained TypeVar's constraint types"
                                .to_owned(),
                        ),
                    });
                }
            }

            if !ref_info.constraint_names.is_empty() {
                // Both have constraints — info's must be a superset
                let info_set: HashSet<&str> =
                    info.constraint_names.iter().map(String::as_str).collect();
                let ref_set: HashSet<&str> = ref_info
                    .constraint_names
                    .iter()
                    .map(String::as_str)
                    .collect();

                if !ref_set.is_subset(&info_set) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "TypeVar `{}`'s constraints `{{{}}}` are not a superset of \
                             default TypeVar `{}`'s constraints `{{{}}}`",
                            info.name,
                            info.constraint_names.join(", "),
                            default_name,
                            ref_info.constraint_names.join(", ")
                        ),
                        span: tv.span,
                        path: path.to_owned(),
                        help: Some(
                            "The constrained TypeVar's constraints must include all of the \
                             default TypeVar's constraints"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 696: the constraints of T2 must be a superset of the \
                             constraints of T1 when T2 defaults to T1"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Check subscripted generic class calls where literal arguments mismatch
/// the resolved parameter types (including `TypeVar` defaults).
///
/// Detects patterns like `Foo[int](1, "")` where `Foo.__init__` expects
/// `(a: int, b: int)` after resolving defaults, but receives a `str` literal.
fn check_subscripted_class_calls(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Build class info map: name -> (generic_params, init_params)
    let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
        .classes
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // Build method info: class_name -> init function info
    let init_map: HashMap<&str, &basilisk_resolver::FunctionInfo> = module
        .functions
        .iter()
        .filter(|f| f.name == "__init__")
        .filter_map(|f| f.class_name.as_deref().map(|cn| (cn, f)))
        .collect();

    // Scan source for `ClassName[args](call_args)` patterns
    for (line_idx, line) in module.source.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        // Look for patterns: Name[type_args](call_args)
        for (class_name, class_info) in &class_map {
            if class_info.generic_params.is_empty() {
                continue;
            }

            let pattern = format!("{class_name}[");
            let Some(start) = trimmed.find(&pattern) else {
                continue;
            };

            let after_name = &trimmed[start + pattern.len()..];
            let Some(bracket_end) = find_matching_bracket(after_name, '[', ']') else {
                continue;
            };

            let type_args_str = &after_name[..bracket_end];
            let after_bracket = &after_name[bracket_end + 1..];

            // Must be followed by (
            if !after_bracket.starts_with('(') {
                continue;
            }

            let call_args_str = &after_bracket[1..];
            let Some(paren_end) = find_matching_bracket(call_args_str, '(', ')') else {
                continue;
            };
            let call_args_str = &call_args_str[..paren_end];

            // Parse type args
            let type_args_owned: Vec<String> = split_top_level_args(type_args_str)
                .iter()
                .map(|s| s.trim().to_owned())
                .collect();
            let type_args: Vec<&str> = type_args_owned.iter().map(String::as_str).collect();

            // Resolve generic params from type args + defaults
            let resolved_types =
                resolve_generic_params(&class_info.generic_params, &type_args, info_map);

            // Get __init__ params (skip self)
            let Some(init_fn) = init_map.get(class_name) else {
                continue;
            };
            let init_params: Vec<_> = init_fn
                .parameters
                .iter()
                .filter(|p| p.name != "self")
                .collect();

            // Parse call args and check against resolved types
            let call_args = split_top_level_args(call_args_str);
            for (arg_idx, call_arg) in call_args.iter().enumerate() {
                let call_arg = call_arg.trim();
                let Some(param) = init_params.get(arg_idx) else {
                    break;
                };

                // Get the param's annotation text
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = module
                    .source
                    .get(ann_span.start as usize..ann_span.end as usize)
                else {
                    continue;
                };

                // Resolve the annotation through the generic mapping
                let resolved_type = resolved_types
                    .get(ann_text)
                    .map_or(ann_text, String::as_str);

                // Check literal arg compatibility
                if let Some(mismatch) = literal_type_mismatch(call_arg, resolved_type) {
                    let byte_offset: u32 = u32::try_from(
                        module
                            .source
                            .lines()
                            .take(line_idx)
                            .map(|l| l.len() + 1)
                            .sum::<usize>(),
                    )
                    .unwrap_or(u32::MAX);
                    let line_len = u32::try_from(line.len()).unwrap_or(u32::MAX);

                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "{class_name}[{type_args_str}].__init__ parameter `{}` expects \
                             `{resolved_type}` but received {mismatch}",
                            param.name
                        ),
                        span: basilisk_resolver::Span {
                            start: byte_offset,
                            end: byte_offset + line_len,
                        },
                        path: module.path.clone(),
                        help: Some(format!(
                            "Pass a value of type `{resolved_type}` for parameter `{}`",
                            param.name
                        )),
                        note: Some(
                            "PEP 696: TypeVar defaults are resolved when the class is \
                             subscripted with fewer type arguments"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Find the matching closing bracket, accounting for nesting.
fn find_matching_bracket(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1i32;
    for (idx, ch) in text.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

/// Resolve generic parameters from explicit type args and defaults.
///
/// Returns a map from `TypeVar` name to resolved concrete type name.
fn resolve_generic_params(
    generic_params: &[basilisk_resolver::GenericParamInfo],
    type_args: &[&str],
    info_map: &HashMap<&str, &TypeVarInfo>,
) -> HashMap<String, String> {
    let mut resolved: HashMap<String, String> = HashMap::new();

    // First, assign explicit type args
    for (idx, param) in generic_params.iter().enumerate() {
        if let Some(&type_arg) = type_args.get(idx) {
            resolved.insert(param.name.clone(), type_arg.to_owned());
        }
    }

    // Then resolve defaults for remaining params
    for param in generic_params {
        if resolved.contains_key(&param.name) {
            continue;
        }

        if let Some(info) = info_map.get(param.name.as_str()) {
            if let Some(ref default_name) = info.default_typevar_name {
                // The default references another TypeVar — resolve it
                if let Some(resolved_type) = resolved.get(default_name.as_str()) {
                    resolved.insert(param.name.clone(), resolved_type.clone());
                }
            }
        }
    }

    resolved
}

/// Check if a literal argument is incompatible with the expected type.
fn literal_type_mismatch(arg: &str, expected_type: &str) -> Option<&'static str> {
    let expected = expected_type.trim().to_ascii_lowercase();

    // Detect literal type from the arg text
    if arg.starts_with('"') || arg.starts_with('\'') {
        // String literal
        match expected.as_str() {
            "int" | "float" | "bool" | "bytes" => Some("a `str` literal"),
            _ => None,
        }
    } else if arg.parse::<i64>().is_ok()
        || (arg.starts_with('-') && arg[1..].parse::<i64>().is_ok())
    {
        // Integer literal
        match expected.as_str() {
            "str" | "bytes" => Some("an `int` literal"),
            _ => None,
        }
    } else if arg.contains('.') && arg.parse::<f64>().is_ok() {
        // Float literal
        match expected.as_str() {
            "int" | "str" | "bool" => Some("a `float` literal"),
            _ => None,
        }
    } else {
        None
    }
}
