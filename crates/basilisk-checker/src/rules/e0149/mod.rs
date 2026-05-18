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

mod helpers;
mod violations;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

use helpers::leading_indent;
use violations::{
    check_decorator_uses_class_type_param, check_method_redefines_class_type_param,
    check_module_level_type_param_use, check_pep695_bound_cross_references,
    check_type_stmt_circular, check_type_stmt_in_function, check_type_stmt_uses_old_typevar,
    collect_pep695_type_params,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0149",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0149",
};

/// Emits BSK-E0149 for PEP 695 generic type parameter scoping violations.
pub(crate) struct Pep695TypeParamScopingViolation;

impl Rule for Pep695TypeParamScopingViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let lines: Vec<&str> = source.lines().collect();

        // Collect all PEP 695 type params defined anywhere in the file.
        let all_pep695_params = collect_pep695_type_params(source);

        // Collect old-style TypeVar names (from TypeVar() calls) for
        // old/new mixing detection.
        let old_typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Track multi-line def/class signatures so continuation lines at
        // indent 0 are not treated as module-level code.
        let mut skip_module_check_until: usize = 0;

        for (line_idx, &line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // --- Violation 1: cross-references in type param bounds ---
            let is_def_or_class = trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ");
            if is_def_or_class {
                check_pep695_bound_cross_references(line, line_number, source, path, diagnostics);

                // For multi-line signatures, skip module-level checks on
                // continuation lines.  Find the `:` that ends the header.
                let code_before_comment = trimmed.split_once('#').map_or(trimmed, |(c, _)| c);
                if !code_before_comment.trim_end().ends_with(':') {
                    for (scan_offset, scan_line) in lines.iter().enumerate().skip(line_idx + 1) {
                        let scan_code = scan_line.split_once('#').map_or(*scan_line, |(c, _)| c);
                        if scan_code.contains(':') {
                            skip_module_check_until = scan_offset + 1;
                            break;
                        }
                    }
                }
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
            // Skip continuation lines of multi-line def/class signatures.
            let in_multiline_sig = line_idx < skip_module_check_until;
            if !in_multiline_sig && leading_indent(line) == 0 && !all_pep695_params.is_empty() {
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

            // --- Violation 4: `type` statement uses old-style TypeVar ---
            if trimmed.starts_with("type ") && !old_typevar_names.is_empty() {
                check_type_stmt_uses_old_typevar(
                    line,
                    line_number,
                    &old_typevar_names,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 5: `type` statement inside function body ---
            if trimmed.starts_with("type ") {
                check_type_stmt_in_function(line, line_number, source, path, diagnostics);
            }

            // --- Violation 6: circular type alias definition ---
            if trimmed.starts_with("type ") {
                check_type_stmt_circular(line, line_number, source, path, diagnostics);
            }
        }

        // --- Violation 7: misuse of PEP 695 type aliases ---
        check_type_alias_misuse(module, diagnostics);

        // --- Violation 8: type argument violates type parameter bound ---
        check_type_alias_bound_violations(module, diagnostics);
    }
}

/// Check for misuse of PEP 695 type aliases: calling, subclassing,
/// `isinstance()`, and attribute access (except `__value__` / `__type_params__`).
fn check_type_alias_misuse(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Collect PEP 695 type alias names
    let alias_names: std::collections::HashSet<&str> = module
        .type_statements
        .iter()
        .map(|ts| ts.name.as_str())
        .collect();

    if alias_names.is_empty() {
        return;
    }

    // Check calls: `Alias()` is not allowed
    for call in &module.calls {
        if alias_names.contains(call.callee.as_str()) {
            // `isinstance(x, Alias)` — special case
            if call.callee == "isinstance" || call.callee == "issubclass" {
                continue; // Handled below via argument checking
            }
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot call type alias `{}`: type aliases are not callable",
                    call.callee
                ),
                call.span,
                path,
                Some("Type aliases created with `type` cannot be instantiated".to_owned()),
                None,
            ));
        }

        // Check isinstance/issubclass with alias as second arg
        if (call.callee == "isinstance" || call.callee == "issubclass") && call.args.len() >= 2 {
            if let Some((_, arg_span)) = call.args.get(1) {
                if let Some(arg_text) = crate::span_util::slice_span(source, *arg_span) {
                    let arg_trimmed = arg_text.trim();
                    if alias_names.contains(arg_trimmed) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Cannot use type alias `{arg_trimmed}` in `{}`",
                                call.callee
                            ),
                            *arg_span,
                            path,
                            Some(format!(
                                "Type aliases created with `type` cannot be used with `{}`",
                                call.callee
                            )),
                            None,
                        ));
                    }
                }
            }
        }
    }

    // Check class bases: `class Foo(Alias)` is not allowed
    for class in &module.classes {
        for base in &class.bases {
            let base_name = base.split('[').next().unwrap_or(base).trim();
            if alias_names.contains(base_name) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Cannot use type alias `{base_name}` as a base class"),
                    class.name_span,
                    path,
                    Some("Type aliases created with `type` cannot be subclassed".to_owned()),
                    None,
                ));
            }
        }
    }

    check_alias_attribute_access(&alias_names, source, path, diagnostics);
}

/// Check attribute access on type aliases (source-level scan).
///
/// Only `__value__` and `__type_params__` are allowed.
/// A PEP 695 type parameter with an optional bound.
struct TypeParamWithBound {
    /// The parameter name (e.g. `S`).
    name: String,
    /// The bound type name (e.g. `int` from `S: int`), if any.
    bound: Option<String>,
    /// Whether this is a `ParamSpec` (`**P`) — these don't have bounds.
    is_paramspec: bool,
    /// Whether this is a `TypeVarTuple` (`*Ts`) — these don't have bounds.
    is_typevartuple: bool,
}

/// A PEP 695 type alias definition with its type parameters and bounds.
struct TypeAliasWithBounds {
    /// The alias name.
    name: String,
    /// Type parameters in declaration order.
    params: Vec<TypeParamWithBound>,
}

/// Collect PEP 695 `type` statements with their type parameter bounds.
fn collect_type_alias_bounds(source: &str) -> Vec<TypeAliasWithBounds> {
    let mut result = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("type ") {
            continue;
        }
        let after_type = &trimmed["type ".len()..];
        let name_end = after_type
            .find(['[', '=', ' ', ':'])
            .unwrap_or(after_type.len());
        let alias_name = after_type[..name_end].trim();
        if alias_name.is_empty() {
            continue;
        }

        let Some(bracket_start) = trimmed.find('[') else {
            continue; // No type params
        };
        let eq_pos = trimmed.find('=').unwrap_or(trimmed.len());
        if bracket_start >= eq_pos {
            continue; // `[` is in RHS, not params
        }

        let after_bracket = &trimmed[bracket_start + 1..];
        let mut depth = 1usize;
        let mut end_idx = 0;
        for (idx, ch) in after_bracket.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = idx;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params_text = &after_bracket[..end_idx];

        // Split on top-level commas
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

        let params: Vec<TypeParamWithBound> = raw_specs
            .iter()
            .map(|spec| {
                let is_paramspec = spec.starts_with("**");
                let is_typevartuple = !is_paramspec && spec.starts_with('*');
                let stripped = spec.trim_start_matches('*');
                let (name, bound) = if let Some(colon_pos) = stripped.find(':') {
                    let name = stripped[..colon_pos].trim().to_owned();
                    let bound = stripped[colon_pos + 1..].trim().to_owned();
                    (name, if bound.is_empty() { None } else { Some(bound) })
                } else {
                    (stripped.trim().to_owned(), None)
                };
                TypeParamWithBound {
                    name,
                    bound,
                    is_paramspec,
                    is_typevartuple,
                }
            })
            .collect();

        if params.iter().any(|p| p.bound.is_some()) {
            result.push(TypeAliasWithBounds {
                name: alias_name.to_owned(),
                params,
            });
        }
    }
    result
}

/// Primitive subtype relationships for bound checking.
///
/// Returns `true` if `arg_type` is a subtype of `bound_type` for the purposes
/// of type parameter bound checking.
fn is_subtype_of(arg_type: &str, bound_type: &str) -> bool {
    arg_type == bound_type
        || matches!(
            (arg_type, bound_type),
            ("bool", "int" | "float" | "complex")
                | ("int", "float" | "complex")
                | ("float", "complex")
        )
}

/// Check type argument bounds when PEP 695 type aliases with bounded params
/// are used in annotations.
fn check_type_alias_bound_violations(
    module: &basilisk_resolver::ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    let aliases = collect_type_alias_bounds(source);
    if aliases.is_empty() {
        return;
    }

    // Scan all module-level annotated variables for usage of these aliases
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }
        let Some(ann_text) = crate::span_util::slice_span(source, var.name_span)
            .and_then(|_| extract_annotation_for_var(source, var.name_span))
        else {
            continue;
        };
        check_annotation_bounds(ann_text, var.name_span, &aliases, source, path, diagnostics);
    }

    // Also check function-local annotated variables
    for func in &module.functions {
        for var in &func.local_vars {
            if !var.has_annotation {
                continue;
            }
            let Some(ann_text) = extract_annotation_for_var(source, var.name_span) else {
                continue;
            };
            check_annotation_bounds(ann_text, var.name_span, &aliases, source, path, diagnostics);
        }
    }
}

/// Extract annotation text from the source line containing the variable name.
fn extract_annotation_for_var(source: &str, name_span: basilisk_resolver::Span) -> Option<&str> {
    let start = usize::try_from(name_span.start).ok()?;
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |pos| start + pos);
    let line = source.get(line_start..line_end)?;
    let name_offset = start.checked_sub(line_start)?;
    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2;
    let annotation_end = line
        .get(after_colon..)?
        .find('=')
        .map_or(line.len(), |p| after_colon + p);
    let annotation = line.get(after_colon..annotation_end)?.trim();
    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

/// Check a single annotation for type argument bound violations.
fn check_annotation_bounds(
    annotation: &str,
    span: basilisk_resolver::Span,
    aliases: &[TypeAliasWithBounds],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Extract base name and type args from annotation like `AliasName[int, str, ...]`
    let Some(bracket_pos) = annotation.find('[') else {
        return;
    };
    let base_name = annotation[..bracket_pos].trim();

    let Some(alias) = aliases.iter().find(|a| a.name == base_name) else {
        return;
    };

    // Extract type arguments
    let after_bracket = &annotation[bracket_pos + 1..];
    let Some(close_bracket) = after_bracket.rfind(']') else {
        return;
    };
    let args_text = &after_bracket[..close_bracket];

    // Split on top-level commas
    let mut args: Vec<&str> = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in args_text.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(args_text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = args_text[start..].trim();
    if !remainder.is_empty() {
        args.push(remainder);
    }

    // Check each arg against the corresponding param's bound
    for (idx, param) in alias.params.iter().enumerate() {
        // Skip ParamSpec / TypeVarTuple — they don't have bounds
        if param.is_paramspec || param.is_typevartuple {
            continue;
        }
        let Some(bound) = &param.bound else {
            continue;
        };
        let Some(arg) = args.get(idx) else {
            continue;
        };
        let arg_trimmed = arg.trim();

        // `...` is valid for ParamSpec args
        if arg_trimmed == "..." {
            continue;
        }

        if !is_subtype_of(arg_trimmed, bound) {
            let line_number = source[..usize::try_from(span.start).unwrap_or(0)]
                .chars()
                .filter(|&c| c == '\n')
                .count()
                + 1;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type argument `{arg_trimmed}` is not compatible with type parameter \
                     `{}` bound `{bound}` in type alias `{base_name}`",
                    param.name
                ),
                helpers::span_for_line(source, line_number),
                path,
                Some(format!(
                    "Type parameter `{}` requires a subtype of `{bound}`",
                    param.name
                )),
                Some(format!(
                    "PEP 695: `{arg_trimmed}` is not a subtype of `{bound}`"
                )),
            ));
        }
    }
}

fn check_alias_attribute_access(
    alias_names: &std::collections::HashSet<&str>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = source.lines().collect();
    for (line_idx, &line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();
        let before_comment = trimmed.split_once('#').map_or(trimmed, |(code, _)| code);

        for alias_name in alias_names {
            let pattern = format!("{alias_name}.");
            if let Some(pos) = before_comment.find(&pattern) {
                let after_dot = &before_comment[pos + pattern.len()..];
                let attr: String = after_dot
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                if attr == "__value__" || attr == "__type_params__" {
                    continue;
                }

                if trimmed.starts_with("type ") {
                    continue;
                }

                if !attr.is_empty() {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Cannot access attribute `{attr}` on type alias `{alias_name}`"
                        ),
                        helpers::span_for_line(source, line_number),
                        path,
                        Some(
                            "Type aliases only support `__value__` and `__type_params__` \
                             attributes"
                                .to_string(),
                        ),
                        None,
                    ));
                }
            }
        }
    }
}
