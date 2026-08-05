//! Implements [`generics_variance_inference`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Variance inference for `generics_variance_inference`.
//! Implements [TYPEINF-GENERICS-VARIANCE].
//!
//! Implements PEP 695 automatic variance inference for type parameters.
//!
//! Variance rules:
//! - **Covariant**: type param appears only in return/output positions
//! - **Contravariant**: type param appears only in parameter/input positions
//! - **Invariant**: type param appears in both, or in mutable containers
//!
//! `__init__` and `__new__` parameters are excluded from variance analysis.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use crate::rules::shared::{contains_typevar_reference, parse_subscript_annotation};

use super::utils::extract_pep695_type_params_ordered;
use super::variance_check::{
    check_fn_body_assignments, check_module_assignments, split_top_level_params,
};

/// Variance of a type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Variance {
    Covariant,
    Contravariant,
    Invariant,
}

/// A generic class collected for variance analysis.
struct ClassForVariance {
    name: String,
    type_params: Vec<String>,
    bases: Vec<String>,
    body_lines: Vec<String>,
}

/// Known invariant base classes (builtin mutable containers).
fn is_known_invariant_base(name: &str) -> bool {
    matches!(name, "list" | "dict" | "set" | "bytearray")
}

/// Known covariant base classes (builtin read-only containers).
fn is_known_covariant_base(name: &str) -> bool {
    matches!(name, "frozenset")
}

/// Extract implicit type params from bases like `class Foo(dict[K, V]):`.
fn extract_implicit_type_params(
    class_line: &str,
    infer_tvs: &HashMap<String, Variance>,
) -> Vec<String> {
    let trimmed = class_line.trim();
    let Some(open) = trimmed.find('(') else {
        return Vec::new();
    };
    let close = trimmed.rfind(')').unwrap_or(trimmed.len());

    let mut params = Vec::new();
    for base in split_top_level_params(&trimmed[open + 1..close]) {
        if let Some((_, args)) = parse_subscript_annotation(base.trim()) {
            for arg in &args {
                let arg = arg.trim();
                if infer_tvs.contains_key(arg) && !params.contains(&arg.to_owned()) {
                    params.push(arg.to_owned());
                }
            }
        }
    }
    params
}

/// Collect all generic classes needing variance inference from source.
fn collect_classes(lines: &[&str], infer_tvs: &HashMap<String, Variance>) -> Vec<ClassForVariance> {
    let mut classes = Vec::new();

    for (idx, &raw_line) in lines.iter().enumerate() {
        let trimmed = raw_line.trim();
        if !trimmed.starts_with("class ") {
            continue;
        }

        let after_class = &trimmed[6..];
        let pep695 = extract_pep695_type_params_ordered(trimmed);

        let (type_params, is_pep695) = if !pep695.is_empty() {
            (pep695, true)
        } else {
            let implicit = extract_implicit_type_params(trimmed, infer_tvs);
            if implicit.is_empty() {
                continue;
            }
            (implicit, false)
        };

        let name_end = after_class
            .find(['[', '(', ':'])
            .unwrap_or(after_class.len());
        let name = after_class[..name_end].trim().to_owned();
        let bases = extract_bases(trimmed, is_pep695);
        let class_indent = raw_line.len() - raw_line.trim_start().len();
        let body_lines = collect_body(lines, idx + 1, class_indent);

        classes.push(ClassForVariance {
            name,
            type_params,
            bases,
            body_lines,
        });
    }
    classes
}

/// Extract base class expressions.
fn extract_bases(class_line: &str, is_pep695: bool) -> Vec<String> {
    let after_class = &class_line.trim()[6..];
    let text = if is_pep695 {
        after_class.find(']').map_or("", |i| &after_class[i + 1..])
    } else {
        after_class
    };
    let Some(open) = text.find('(') else {
        return Vec::new();
    };
    let close = text.rfind(')').unwrap_or(text.len());
    split_top_level_params(&text[open + 1..close])
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Collect trimmed class body lines.
fn collect_body(lines: &[&str], start: usize, class_indent: usize) -> Vec<String> {
    let mut body = Vec::new();
    for &line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if (line.len() - line.trim_start().len()) <= class_indent {
            break;
        }
        body.push(trimmed.to_owned());
    }
    body
}

/// Infer variance for a single type parameter from its class body.
fn infer_param_variance(
    param: &str,
    class: &ClassForVariance,
    known: &HashMap<String, Vec<Variance>>,
) -> Variance {
    let (mut cov, mut contra) = (false, false);

    // Check base class constraints
    for base in &class.bases {
        if let Some((name, args)) = parse_subscript_annotation(base) {
            for (pos, arg) in args.iter().enumerate() {
                if !contains_typevar_reference(arg, param) {
                    continue;
                }
                if is_known_invariant_base(name) {
                    return Variance::Invariant;
                }
                if is_known_covariant_base(name) {
                    cov = true;
                    continue;
                }
                if let Some(vars) = known.get(name) {
                    match vars.get(pos) {
                        Some(Variance::Invariant) => return Variance::Invariant,
                        Some(Variance::Covariant) => cov = true,
                        Some(Variance::Contravariant) => contra = true,
                        None => {}
                    }
                }
            }
        }
    }

    // Scan body for usage positions
    let mut in_init = false;
    let mut has_setter = false;
    let mut has_mutable_attr = false;

    for line in &class.body_lines {
        if line.contains(".setter") {
            has_setter = true;
        }
        if let Some(after_def) = line.strip_prefix("def ") {
            let method = after_def.split('(').next().unwrap_or("").trim();
            let excluded = method == "__init__" || method == "__new__";
            in_init = excluded;
            if !excluded {
                scan_method_sig(line, param, &mut cov, &mut contra);
            }
            continue;
        }
        // Public attr assignment in __init__ (skip private fields)
        if in_init && line.contains('=') && !line.contains("==") {
            let attr = line
                .strip_prefix("self.")
                .and_then(|rest| rest.split(['=', '.', ' ']).next())
                .unwrap_or("")
                .trim();
            if !attr.starts_with('_') && !attr.is_empty() {
                has_mutable_attr = true;
            }
        }
    }

    if has_mutable_attr {
        return Variance::Invariant;
    }
    if has_setter && cov {
        return Variance::Invariant;
    }
    match (cov, contra) {
        (true, true) | (false, false) => Variance::Invariant,
        (true, false) => Variance::Covariant,
        (false, true) => Variance::Contravariant,
    }
}

/// Scan a method signature for type param in return/parameter positions.
fn scan_method_sig(line: &str, param: &str, cov: &mut bool, contra: &mut bool) {
    if let Some(arrow) = line.find("->") {
        let ret = line[arrow + 2..].split(':').next().unwrap_or("").trim();
        if contains_typevar_reference(ret, param) {
            *cov = true;
        }
    }
    let Some(open) = line.find('(') else { return };
    let Some(close) = line.rfind(')') else { return };
    for (i, p) in line[open + 1..close].split(',').enumerate() {
        let p = p.trim();
        if i == 0 && (p.starts_with("self") || p.starts_with("cls")) {
            continue;
        }
        if let Some(c) = p.find(':') {
            if contains_typevar_reference(p[c + 1..].split('=').next().unwrap_or(""), param) {
                *contra = true;
            }
        }
    }
}

/// Main entry point: check variance-related assignment violations.
pub(super) fn check_variance_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = module.source.lines().collect();
    let mut infer_tvs: HashMap<String, Variance> = HashMap::new();

    for tv in &module.typevar_calls {
        let var = if tv.is_covariant {
            Variance::Covariant
        } else if tv.is_contravariant {
            Variance::Contravariant
        } else {
            Variance::Invariant
        };
        if tv.has_infer_variance {
            let _ = infer_tvs.insert(tv.name.clone(), var);
        }
    }

    let mut known: HashMap<String, Vec<Variance>> = HashMap::new();

    // Infer variances for classes that need it (PEP 695, infer_variance).
    let classes = collect_classes(&lines, &infer_tvs);
    for _ in 0..2 {
        for class in &classes {
            let vars: Vec<Variance> = class
                .type_params
                .iter()
                .map(|p| infer_param_variance(p, class, &known))
                .collect();
            let _ = known.insert(class.name.clone(), vars);
        }
    }

    if known.is_empty() {
        return;
    }

    // Variance verdicts route through the module-seeded context
    // ([NARROWPLAN-SUBTYPING]).
    let subtyping = crate::subtyping::module_context(module);
    check_module_assignments(
        &subtyping,
        &lines,
        &known,
        &module.source,
        &module.path,
        diagnostics,
    );
    check_fn_body_assignments(
        &subtyping,
        &lines,
        &known,
        &module.source,
        &module.path,
        diagnostics,
    );
}
