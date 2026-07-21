//! Implements [`generics_variance_inference`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Method-call and constructor-call type checking for `generics_variance_inference`.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use crate::rules::shared::{
    is_type_compatible, parse_subscript_annotation, split_top_level_commas,
};

use super::collect::{collect_generic_classes, collect_generic_instances};
use super::utils::{find_matching_close, infer_literal_type, span_for_line};

const CODE: ErrorCode = ErrorCode {
    code: "generics_variance_inference",
    docs_url: "https://www.basilisk-python.dev/errors/generics_variance_inference",
};

/// Check module-level method calls on generic class instances for type mismatches.
#[expect(
    clippy::too_many_lines,
    reason = "method call validation requires many steps"
)]
pub(super) fn check_generic_instance_method_calls(
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

    // Build a map: var_name -> (class_def, typevar_substitution_map)
    let mut var_substitutions: HashMap<
        String,
        (&super::types::GenericClassDef, HashMap<String, String>),
    > = HashMap::new();

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
            let _ = var_substitutions.insert(instance.var_name.clone(), (class_def, subst));
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
            let args: Vec<String> = split_top_level_commas(args_text)
                .iter()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();

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

                if !is_type_compatible(actual_type, expected_type) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
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
                        span_for_line(source, line_number),
                        path,
                        Some(format!(
                            "Parameter `{param_name}` expects `{expected_type}` \
                             because `{typevar_name}` is bound to `{expected_type}` \
                             for this instance"
                        )),
                        Some(
                            "PEP 484: type variables in methods of generic classes \
                             are bound to the class's type arguments"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}

/// Check constructor calls with partially-specialized generic classes.
///
/// Detects `ClassName[TypeArgs](args)` patterns where `TypeVar` defaults
/// fill in missing type arguments, and validates argument types.
pub(super) fn check_generic_constructor_calls(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let generic_classes = collect_generic_classes(&module.source);
    if generic_classes.is_empty() {
        return;
    }

    // Build TypeVar default map: name → default_type_name
    let tv_defaults: HashMap<String, String> = module
        .typevar_calls
        .iter()
        .filter_map(|tv| {
            tv.default_type_name
                .as_ref()
                .map(|d| (tv.name.clone(), d.clone()))
        })
        .collect();

    if tv_defaults.is_empty() {
        return;
    }

    let lines: Vec<&str> = module.source.lines().collect();
    let mut fn_param_types: HashMap<String, String> = HashMap::new();
    let mut in_fn = false;
    let mut fn_indent = 0usize;

    for (line_idx, &line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Track function parameter types
        if trimmed.starts_with("def ") {
            in_fn = true;
            fn_indent = indent;
            fn_param_types.clear();
            if let (Some(open), Some(close)) = (trimmed.find('('), trimmed.rfind(')')) {
                for p in trimmed[open + 1..close].split(',') {
                    let p = p.trim();
                    if let Some(colon) = p.find(':') {
                        let name = p[..colon].trim();
                        let ann = p[colon + 1..].split('=').next().unwrap_or("").trim();
                        if name != "self" && name != "cls" {
                            let _ = fn_param_types.insert(name.to_owned(), ann.to_owned());
                        }
                    }
                }
            }
            continue;
        }

        if in_fn && indent <= fn_indent && !trimmed.is_empty() {
            in_fn = false;
            fn_param_types.clear();
        }

        if !in_fn || fn_param_types.is_empty() {
            continue;
        }

        // Look for ClassName[TypeArgs](args) pattern
        // Strip comments to avoid brackets in comments breaking annotation parsing
        let code_part = trimmed.split('#').next().unwrap_or(trimmed).trim();
        for class_def in &generic_classes {
            let pattern = format!("{}[", class_def.name);
            let Some(start) = code_part.find(pattern.as_str()) else {
                continue;
            };
            let after_name = &code_part[start + class_def.name.len()..];

            // Parse [TypeArgs]
            let Some((_, explicit_args)) = parse_subscript_annotation(&code_part[start..]) else {
                continue;
            };

            // Partial specialization: fewer args than type params → fill defaults
            if explicit_args.len() >= class_def.typevar_params.len() {
                continue; // fully specialized, no default issues
            }

            let resolved =
                resolve_with_defaults(&class_def.typevar_params, &explicit_args, &tv_defaults);

            // Find the args to the constructor call: after ](
            let bracket_end = after_name.find("](");
            let Some(bracket_end) = bracket_end else {
                continue;
            };
            let call_args_start = &after_name[bracket_end + 2..];
            let close_paren = find_matching_close(call_args_start);
            let call_args_text = &call_args_start[..close_paren];
            let call_args: Vec<String> = split_top_level_commas(call_args_text)
                .iter()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();

            // Check __init__ params against call args
            let Some(init_params) = class_def.methods.get("__init__") else {
                continue;
            };

            check_constructor_args(
                &ConstructorCheckCtx {
                    class_def,
                    init_params,
                    resolved: &resolved,
                    call_args: &call_args,
                    fn_param_types: &fn_param_types,
                    source: &module.source,
                    path: &module.path,
                    line_number,
                },
                diagnostics,
            );
        }
    }
}

/// Resolve type args with defaults for a partially-specialized generic class.
fn resolve_with_defaults(
    type_params: &[String],
    explicit_args: &[String],
    tv_defaults: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut resolved: HashMap<String, String> = HashMap::new();

    for (idx, param) in type_params.iter().enumerate() {
        if let Some(explicit) = explicit_args.get(idx) {
            let _ = resolved.insert(param.clone(), explicit.clone());
        } else if let Some(default_tv) = tv_defaults.get(param) {
            // Default might reference another TypeVar that's already resolved
            let resolved_default = resolved
                .get(default_tv)
                .cloned()
                .unwrap_or_else(|| default_tv.clone());
            let _ = resolved.insert(param.clone(), resolved_default);
        }
    }

    resolved
}

/// Context for checking constructor arguments against resolved types.
struct ConstructorCheckCtx<'a> {
    class_def: &'a super::types::GenericClassDef,
    init_params: &'a [(String, String)],
    resolved: &'a HashMap<String, String>,
    call_args: &'a [String],
    fn_param_types: &'a HashMap<String, String>,
    source: &'a str,
    path: &'a str,
    line_number: usize,
}

/// Check constructor arguments against resolved `__init__` parameter types.
fn check_constructor_args(ctx: &ConstructorCheckCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for (param_idx, (param_name, param_ann)) in ctx.init_params.iter().enumerate() {
        let typevar_name = param_ann.trim();
        let Some(expected_type) = ctx.resolved.get(typevar_name) else {
            continue;
        };

        let Some(arg_raw) = ctx.call_args.get(param_idx) else {
            continue;
        };
        let arg_name = arg_raw.trim();

        // Try to resolve the argument's type from function params
        let actual_type = ctx
            .fn_param_types
            .get(arg_name)
            .map(String::as_str)
            .or_else(|| infer_literal_type(arg_name));

        let Some(actual_type) = actual_type else {
            continue;
        };

        if !is_type_compatible(actual_type, expected_type) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument `{arg_name}` of type `{actual_type}` is not compatible \
                     with parameter `{param_name}: {expected_type}` in \
                     `{class_name}.__init__` (`TypeVar` default propagation)",
                    class_name = ctx.class_def.name
                ),
                span_for_line(ctx.source, ctx.line_number),
                ctx.path,
                Some(format!(
                    "`TypeVar` `{typevar_name}` resolves to `{expected_type}` \
                     via default propagation"
                )),
                Some(
                    "PEP 696: `TypeVar` defaults propagate when fewer type \
                     arguments are provided than type parameters"
                        .to_owned(),
                ),
            ));
        }
    }
}
