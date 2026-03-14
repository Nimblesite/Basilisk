//! Method-call type checking for BSK-E0130.

use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::collect::{collect_generic_classes, collect_generic_instances};
use super::utils::{
    find_matching_close, generic_types_compatible, infer_literal_type, span_for_line,
    split_top_level_type_args,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0130",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0130",
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
    let mut var_substitutions: HashMap<String, (&super::types::GenericClassDef, HashMap<String, String>)> =
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
