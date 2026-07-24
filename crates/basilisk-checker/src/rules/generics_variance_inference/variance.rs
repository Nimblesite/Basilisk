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

use super::utils::{extract_pep695_type_params_ordered, extract_typevar_params_from_generic};
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
    is_frozen_dataclass: bool,
    is_dataclass: bool,
}

/// Known invariant base classes (mutable containers).
fn is_known_invariant_base(name: &str) -> bool {
    matches!(
        name,
        "list" | "dict" | "set" | "deque" | "bytearray" | "MutableSequence" | "MutableMapping"
    )
}

/// Known covariant base classes (read-only containers).
fn is_known_covariant_base(name: &str) -> bool {
    matches!(
        name,
        "Sequence" | "FrozenSet" | "frozenset" | "Iterator" | "Iterable" | "Mapping"
    )
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
        let generic = extract_typevar_params_from_generic(trimmed);

        let (type_params, is_pep695) = if !pep695.is_empty() {
            (pep695, true)
        } else if !generic.is_empty() && generic.iter().all(|p| infer_tvs.contains_key(p)) {
            (generic, false)
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
        let (is_dc, is_frozen) = check_decorators(lines, idx);
        let class_indent = raw_line.len() - raw_line.trim_start().len();
        let body_lines = collect_body(lines, idx + 1, class_indent);

        classes.push(ClassForVariance {
            name,
            type_params,
            bases,
            body_lines,
            is_frozen_dataclass: is_frozen,
            is_dataclass: is_dc,
        });
    }
    classes
}

/// Extract base class expressions, filtering out `Generic[...]`.
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
        .filter(|s| !s.is_empty() && !s.starts_with("Generic["))
        .collect()
}

/// Check preceding decorators for `@dataclass` variants.
fn check_decorators(lines: &[&str], class_idx: usize) -> (bool, bool) {
    let (mut is_dc, mut is_frozen) = (false, false);
    let mut i = class_idx;
    while i > 0 {
        i -= 1;
        let Some(prev) = lines.get(i).map(|l| l.trim()) else {
            break;
        };
        if prev.starts_with('@') {
            if prev.contains("dataclass") {
                is_dc = true;
                if prev.contains("frozen") && prev.contains("True") {
                    is_frozen = true;
                }
            }
        } else if !prev.is_empty() && !prev.starts_with('#') {
            break;
        }
    }
    (is_dc, is_frozen)
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

    // Collect Final field names (read-only even with self.x = ...)
    let final_fields: std::collections::HashSet<&str> = class
        .body_lines
        .iter()
        .filter_map(|l| {
            if l.starts_with("def ") || l.starts_with('@') || !l.contains(':') {
                return None;
            }
            let (field, ann) = l.split_once(':')?;
            let field = field.trim();
            let ann = ann.split('=').next()?.trim();
            (ann.contains("Final")
                && !field.is_empty()
                && field.chars().all(|c| c.is_alphanumeric() || c == '_'))
            .then_some(field)
        })
        .collect();

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
        // Public attr assignment in __init__ (skip Final and private fields)
        if in_init && line.contains('=') && !line.contains("==") {
            let attr = line
                .strip_prefix("self.")
                .and_then(|rest| rest.split(['=', '.', ' ']).next())
                .unwrap_or("")
                .trim();
            if !attr.starts_with('_') && !attr.is_empty() && !final_fields.contains(attr) {
                has_mutable_attr = true;
            }
        }
        // Class-level field annotations
        if !line.starts_with("def ")
            && !line.starts_with('@')
            && line.contains(':')
            && !line.starts_with("self.")
        {
            scan_field(line, param, class, &mut cov, &mut contra);
        }
    }

    if has_mutable_attr && !class.is_frozen_dataclass {
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

/// Scan a class-level field annotation for variance implications.
fn scan_field(
    line: &str,
    param: &str,
    class: &ClassForVariance,
    cov: &mut bool,
    contra: &mut bool,
) {
    let Some((field, ann)) = line.split_once(':') else {
        return;
    };
    let field = field.trim();
    let ann = ann.split('=').next().unwrap_or(ann).trim();
    if field.is_empty() || !field.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return;
    }
    if !contains_typevar_reference(ann, param) {
        return;
    }
    if ann.contains("Final") || class.is_frozen_dataclass {
        *cov = true;
    } else if class.is_dataclass {
        *cov = true;
        *contra = true;
    }
}

/// Resolve traditional parent classes' Generic `TypeVar` variances.
fn resolve_parent_variances(
    lines: &[&str],
    tv_declared: &HashMap<String, Variance>,
) -> HashMap<String, Vec<Variance>> {
    let mut result = HashMap::new();
    for &line in lines {
        let trimmed = line.trim();
        // Match both `Generic[T]` and `Protocol[T]` bases.
        if !trimmed.starts_with("class ")
            || (!trimmed.contains("Generic[") && !trimmed.contains("Protocol["))
        {
            continue;
        }
        let after = &trimmed[6..];
        let name_end = after.find(['(', '[', ':']).unwrap_or(after.len());
        let name = after[..name_end].trim().to_owned();
        let params = extract_typevar_params_from_generic(trimmed);
        let vars: Vec<Variance> = params
            .iter()
            .map(|p| tv_declared.get(p).copied().unwrap_or(Variance::Invariant))
            .collect();
        let _ = result.insert(name, vars);
    }
    result
}

/// Main entry point: check variance-related assignment violations.
pub(super) fn check_variance_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = module.source.lines().collect();
    let mut infer_tvs: HashMap<String, Variance> = HashMap::new();
    let mut tv_declared: HashMap<String, Variance> = HashMap::new();

    for tv in &module.typevar_calls {
        let var = if tv.is_covariant {
            Variance::Covariant
        } else if tv.is_contravariant {
            Variance::Contravariant
        } else {
            Variance::Invariant
        };
        let _ = tv_declared.insert(tv.name.clone(), var);
        if tv.has_infer_variance {
            let _ = infer_tvs.insert(tv.name.clone(), var);
        }
    }

    // Build known variances: parent classes with explicitly declared variance.
    let mut known: HashMap<String, Vec<Variance>> = resolve_parent_variances(&lines, &tv_declared);

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

    check_module_assignments(&lines, &known, &module.source, &module.path, diagnostics);
    check_fn_body_assignments(&lines, &known, &module.source, &module.path, diagnostics);
}
