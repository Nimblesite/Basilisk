//! Implements [`generics_syntax_scoping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! PEP 695 type-alias misuse (violation 7) and type-argument bound checks
//! (violation 8) for `generics_syntax_scoping`.
//!
//! Attribute accesses and alias type-parameter bounds are sourced from
//! `ruff_python_ast` nodes (via [`basilisk_resolver::Pep695Scoping`]), never
//! from raw line scanning.

use std::collections::HashSet;

use basilisk_resolver::{Pep695ParamKind, Pep695Scoping, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::CODE;

/// A PEP 695 type parameter with its (optional) bound, for bound checking.
struct TypeParamWithBound {
    name: String,
    bound: Option<String>,
    skip_bound: bool,
}

/// A PEP 695 type alias with at least one bounded type parameter.
struct TypeAliasWithBounds {
    name: String,
    params: Vec<TypeParamWithBound>,
}

// ---------------------------------------------------------------------------
// Violation 7: misuse of a PEP 695 type alias
// ---------------------------------------------------------------------------

/// Aliases cannot be called, subclassed, used in `isinstance`/`issubclass`, or
/// have attributes accessed (except `__value__` / `__type_params__`).
pub(super) fn check_type_alias_misuse(
    module: &ResolvedModule,
    scoping: &Pep695Scoping,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    let alias_names: HashSet<&str> = module
        .type_statements
        .iter()
        .map(|ts| ts.name.as_str())
        .collect();
    if alias_names.is_empty() {
        return;
    }

    for call in &module.calls {
        if alias_names.contains(call.callee.as_str())
            && call.callee != "isinstance"
            && call.callee != "issubclass"
        {
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

        if (call.callee == "isinstance" || call.callee == "issubclass") && call.args.len() >= 2 {
            if let Some((_, arg_span)) = call.args.get(1) {
                if let Some(arg_text) = crate::span_util::slice_span(source, *arg_span) {
                    let arg_trimmed = arg_text.trim();
                    if alias_names.contains(arg_trimmed) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!("Cannot use type alias `{arg_trimmed}` in `{}`", call.callee),
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

    check_alias_attribute_access(&alias_names, scoping, path, diagnostics);
}

/// Only `__value__` and `__type_params__` may be accessed on a type alias.
fn check_alias_attribute_access(
    alias_names: &HashSet<&str>,
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for access in &scoping.attr_accesses {
        if !alias_names.contains(access.base.as_str()) {
            continue;
        }
        if access.attr == "__value__" || access.attr == "__type_params__" {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot access attribute `{}` on type alias `{}`",
                access.attr, access.base
            ),
            access.span,
            path,
            Some(
                "Type aliases only support `__value__` and `__type_params__` attributes".to_owned(),
            ),
            None,
        ));
    }
}

// ---------------------------------------------------------------------------
// Violation 8: a type argument violates a type parameter's bound
// ---------------------------------------------------------------------------

/// Primitive subtype relationship used for bound checking.
fn is_subtype_of(arg_type: &str, bound_type: &str) -> bool {
    arg_type == bound_type
        || matches!(
            (arg_type, bound_type),
            ("bool", "int" | "float" | "complex")
                | ("int", "float" | "complex")
                | ("float", "complex")
        )
}

/// Build the bounded-alias table from AST-derived alias definitions.
fn collect_bounded_aliases(scoping: &Pep695Scoping) -> Vec<TypeAliasWithBounds> {
    scoping
        .aliases
        .iter()
        .filter(|alias| alias.params.iter().any(|p| p.bound_text.is_some()))
        .map(|alias| TypeAliasWithBounds {
            name: alias.name.clone(),
            params: alias
                .params
                .iter()
                .map(|p| TypeParamWithBound {
                    name: p.name.clone(),
                    bound: p.bound_text.clone(),
                    skip_bound: matches!(
                        p.kind,
                        Pep695ParamKind::ParamSpec | Pep695ParamKind::TypeVarTuple
                    ),
                })
                .collect(),
        })
        .collect()
}

/// Check type-argument bounds where bounded PEP 695 aliases are used in
/// annotations.
pub(super) fn check_type_alias_bound_violations(
    module: &ResolvedModule,
    scoping: &Pep695Scoping,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    let aliases = collect_bounded_aliases(scoping);
    if aliases.is_empty() {
        return;
    }

    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }
        if let Some(annotation) = extract_annotation_for_var(source, var.name_span) {
            check_annotation_bounds(annotation, var.name_span, &aliases, path, diagnostics);
        }
    }

    for func in &module.functions {
        for var in &func.local_vars {
            if !var.has_annotation {
                continue;
            }
            if let Some(annotation) = extract_annotation_for_var(source, var.name_span) {
                check_annotation_bounds(annotation, var.name_span, &aliases, path, diagnostics);
            }
        }
    }
}

/// Extract the annotation text from the source line containing the variable.
fn extract_annotation_for_var(source: &str, name_span: Span) -> Option<&str> {
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
    (!annotation.is_empty()).then_some(annotation)
}

/// Check a single annotation `AliasName[args...]` for bound violations.
fn check_annotation_bounds(
    annotation: &str,
    span: Span,
    aliases: &[TypeAliasWithBounds],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(bracket_pos) = annotation.find('[') else {
        return;
    };
    let base_name = annotation[..bracket_pos].trim();
    let Some(alias) = aliases.iter().find(|a| a.name == base_name) else {
        return;
    };

    let after_bracket = &annotation[bracket_pos + 1..];
    let Some(close_bracket) = after_bracket.rfind(']') else {
        return;
    };
    let args = split_top_level(&after_bracket[..close_bracket]);

    for (idx, param) in alias.params.iter().enumerate() {
        if param.skip_bound {
            continue;
        }
        let Some(bound) = &param.bound else {
            continue;
        };
        let Some(arg) = args.get(idx) else {
            continue;
        };
        let arg_trimmed = arg.trim();
        if arg_trimmed == "..." || is_subtype_of(arg_trimmed, bound) {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Type argument `{arg_trimmed}` is not compatible with type parameter `{}` bound \
                 `{bound}` in type alias `{base_name}`",
                param.name
            ),
            span,
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

/// Split a comma-separated argument list on top-level commas only.
fn split_top_level(text: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        args.push(remainder);
    }
    args
}
