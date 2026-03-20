//! Helper types and functions for BSK-E0128.
//!
//! Provides source-level parsing of `TypeVar` definitions, argument splitting,
//! numeric subtype checks, bracket matching, generic param resolution, and
//! literal type mismatch detection.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::GenericParamInfo;

use crate::rules::shared::{is_numeric_subtype, split_top_level_commas};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Information about a `TypeVar` extracted from source text.
pub(super) struct TypeVarInfo {
    /// Name of the `TypeVar` (LHS of assignment).
    pub(super) name: String,
    /// Name of the `TypeVar` referenced in `default=`, if any.
    pub(super) default_typevar_name: Option<String>,
    /// The bound type name, if `bound=` is present.
    pub(super) bound_name: Option<String>,
    /// Constraint type names (positional args after the name string).
    pub(super) constraint_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// Source-level parsing
// ---------------------------------------------------------------------------

/// Parse `TypeVar` definitions from source text to extract default value names,
/// bound names, and constraint names that are not available in the resolver's
/// `TypeVarCallInfo`.
pub(super) fn parse_typevar_info_from_source(
    source: &str,
    typevar_names: &HashSet<&str>,
) -> Vec<TypeVarInfo> {
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
        let args = split_top_level_commas(inner);

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


// ---------------------------------------------------------------------------
// Bracket matching
// ---------------------------------------------------------------------------

/// Find the matching closing bracket, accounting for nesting.
pub(super) fn find_matching_bracket(text: &str, open: char, close: char) -> Option<usize> {
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

// ---------------------------------------------------------------------------
// Generic param resolution
// ---------------------------------------------------------------------------

/// Resolve generic parameters from explicit type args and defaults.
///
/// Returns a map from `TypeVar` name to resolved concrete type name.
pub(super) fn resolve_generic_params(
    generic_params: &[GenericParamInfo],
    type_args: &[&str],
    info_map: &HashMap<&str, &TypeVarInfo>,
) -> HashMap<String, String> {
    let mut resolved: HashMap<String, String> = HashMap::new();

    // First, assign explicit type args
    for (idx, param) in generic_params.iter().enumerate() {
        if let Some(&type_arg) = type_args.get(idx) {
            let _ = resolved.insert(param.name.clone(), type_arg.to_owned());
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
                    let _ = resolved.insert(param.name.clone(), resolved_type.clone());
                }
            }
        }
    }

    resolved
}

// ---------------------------------------------------------------------------
// Literal type mismatch
// ---------------------------------------------------------------------------

/// Check if a literal argument is incompatible with the expected type.
pub(super) fn literal_type_mismatch(arg: &str, expected_type: &str) -> Option<&'static str> {
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
