//! Variance inference and assignment checking for BSK-E0130.
//!
//! Implements PEP 695 automatic variance inference for type parameters
//! and checks generic type assignments for variance compatibility.
//!
//! Variance rules:
//! - **Covariant**: type param appears only in return/output positions
//! - **Contravariant**: type param appears only in parameter/input positions
//! - **Invariant**: type param appears in both, or in mutable containers
//!
//! `__init__` and `__new__` parameters are excluded from variance analysis.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::utils::{
    contains_typevar_reference, extract_pep695_type_params,
    extract_typevar_params_from_generic, parse_generic_annotation, span_for_line,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0130",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0130",
};

/// Variance of a type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variance {
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

/// Check if `sub` is a numeric subtype of `sup` in the Python type hierarchy.
fn is_numeric_subtype(sub: &str, sup: &str) -> bool {
    if sub == sup {
        return true;
    }
    matches!(
        (sub, sup),
        ("bool", "int")
            | ("bool", "float")
            | ("bool", "complex")
            | ("int", "float")
            | ("int", "complex")
            | ("float", "complex")
    )
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
        "Sequence"
            | "FrozenSet"
            | "frozenset"
            | "Iterator"
            | "Iterable"
            | "Mapping"
            | "tuple"
            | "Tuple"
    )
}

/// Extract implicit type parameters from a class that inherits from a generic base
/// (e.g., `class Foo(dict[K, V]):`) where the TypeVars have `infer_variance=True`.
fn extract_implicit_type_params(
    class_line: &str,
    infer_variance_tvs: &HashMap<String, Variance>,
) -> Vec<String> {
    let trimmed = class_line.trim();
    let Some(open) = trimmed.find('(') else {
        return Vec::new();
    };
    let Some(close) = trimmed.rfind(')') else {
        return Vec::new();
    };
    let bases_text = &trimmed[open + 1..close];

    // Look for TypeVars with infer_variance in the bases
    let mut params = Vec::new();
    for base in bases_text.split(',') {
        let base = base.trim();
        if let Some(bracket) = base.find('[') {
            let inner = &base[bracket + 1..];
            if let Some(end) = inner.rfind(']') {
                for arg in inner[..end].split(',') {
                    let arg = arg.trim();
                    if infer_variance_tvs.contains_key(arg) && !params.contains(&arg.to_owned()) {
                        params.push(arg.to_owned());
                    }
                }
            }
        }
    }
    params
}

/// Collect all generic classes (PEP 695 + traditional with `infer_variance`) from source.
fn collect_classes_for_variance(
    lines: &[&str],
    infer_variance_tvs: &HashMap<String, Variance>,
) -> Vec<ClassForVariance> {
    let mut classes = Vec::new();
    let mut idx = 0usize;

    while idx < lines.len() {
        let trimmed = lines[idx].trim();

        if !trimmed.starts_with("class ") {
            idx += 1;
            continue;
        }

        let after_class = &trimmed[6..];

        // Try PEP 695: class Name[T1, T2](bases):
        let pep695_params = extract_pep695_type_params(trimmed);

        // Try traditional: class Name(Generic[T]):
        let generic_params = extract_typevar_params_from_generic(trimmed);

        let (type_params, is_pep695) = if !pep695_params.is_empty() {
            (pep695_params.into_iter().collect::<Vec<_>>(), true)
        } else if !generic_params.is_empty() {
            // Only include if ALL params have infer_variance
            let all_infer = generic_params
                .iter()
                .all(|p| infer_variance_tvs.contains_key(p));
            if all_infer {
                (generic_params, false)
            } else {
                idx += 1;
                continue;
            }
        } else {
            // Try implicit generic: class Name(dict[K, V]): or class Name(Sequence[T]):
            // where K, V, T have infer_variance=True
            let implicit_params =
                extract_implicit_type_params(trimmed, infer_variance_tvs);
            if !implicit_params.is_empty() {
                (implicit_params, false)
            } else {
                idx += 1;
                continue;
            }
        };

        // Extract class name
        let name_end = after_class
            .find(|c: char| c == '[' || c == '(' || c == ':')
            .unwrap_or(after_class.len());
        let name = after_class[..name_end].trim().to_owned();

        // Extract bases
        let bases = extract_bases(trimmed, is_pep695);

        // Check for decorators
        let (is_dataclass, is_frozen_dataclass) = check_decorators(lines, idx);

        // Collect class body lines
        let class_indent = lines[idx].len() - lines[idx].trim_start().len();
        let body_lines = collect_body_lines(lines, idx + 1, class_indent);

        classes.push(ClassForVariance {
            name,
            type_params,
            bases,
            body_lines,
            is_frozen_dataclass,
            is_dataclass,
        });

        idx += 1;
    }

    classes
}

/// Extract base class expressions from a class definition line.
fn extract_bases(class_line: &str, is_pep695: bool) -> Vec<String> {
    let trimmed = class_line.trim();
    let after_class = &trimmed[6..]; // skip "class "

    if is_pep695 {
        // PEP 695: class Name[T](bases): — bases come after ]
        let Some(close_bracket) = after_class.find(']') else {
            return Vec::new();
        };
        let after_bracket = &after_class[close_bracket + 1..];
        extract_paren_bases(after_bracket)
    } else {
        // Traditional: class Name(bases):
        extract_paren_bases(after_class)
    }
}

/// Extract bases from `(base1, base2)` portion, filtering out `Generic[...]`.
fn extract_paren_bases(text: &str) -> Vec<String> {
    let Some(open) = text.find('(') else {
        return Vec::new();
    };
    let close = text.rfind(')').unwrap_or(text.len());
    let bases_text = &text[open + 1..close];

    // Split at top-level commas
    let mut bases = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in bases_text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                let base = bases_text[start..i].trim();
                if !base.is_empty() && !base.starts_with("Generic[") {
                    bases.push(base.to_owned());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = bases_text[start..].trim();
    if !last.is_empty() && !last.starts_with("Generic[") {
        bases.push(last.to_owned());
    }
    bases
}

/// Check preceding decorators for `@dataclass` and `@dataclass(frozen=True)`.
fn check_decorators(lines: &[&str], class_idx: usize) -> (bool, bool) {
    let mut is_dc = false;
    let mut is_frozen = false;
    let mut check_idx = class_idx;
    while check_idx > 0 {
        check_idx -= 1;
        let prev = lines[check_idx].trim();
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

/// Collect class body lines (indented deeper than `class_indent`).
fn collect_body_lines(lines: &[&str], start: usize, class_indent: usize) -> Vec<String> {
    let mut body = Vec::new();
    let mut idx = start;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            idx += 1;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent <= class_indent {
            break;
        }
        body.push(trimmed.to_owned());
        idx += 1;
    }
    body
}

/// Infer variance for all type params of a class.
fn infer_variances(
    class: &ClassForVariance,
    known_variances: &HashMap<String, Vec<Variance>>,
    typevar_declared: &HashMap<String, Variance>,
) -> Vec<Variance> {
    class
        .type_params
        .iter()
        .map(|param| infer_single_variance(param, class, known_variances, typevar_declared))
        .collect()
}

/// Infer variance for a single type parameter.
#[expect(
    clippy::too_many_lines,
    reason = "variance inference requires checking many patterns"
)]
fn infer_single_variance(
    param: &str,
    class: &ClassForVariance,
    known_variances: &HashMap<String, Vec<Variance>>,
    typevar_declared: &HashMap<String, Variance>,
) -> Variance {
    let mut covariant_usage = false;
    let mut contravariant_usage = false;

    // Phase 1: Check base class constraints
    for base in &class.bases {
        if let Some((base_name, base_args)) = parse_generic_annotation(base) {
            for (pos, arg) in base_args.iter().enumerate() {
                if !contains_typevar_reference(arg, param) {
                    continue;
                }

                if is_known_invariant_base(&base_name) {
                    return Variance::Invariant;
                }

                if is_known_covariant_base(&base_name) {
                    covariant_usage = true;
                    continue;
                }

                // Check inferred variance from other PEP 695 classes
                if let Some(base_vars) = known_variances.get(&base_name) {
                    if let Some(base_var) = base_vars.get(pos) {
                        match base_var {
                            Variance::Invariant => return Variance::Invariant,
                            Variance::Covariant => covariant_usage = true,
                            Variance::Contravariant => contravariant_usage = true,
                        }
                    }
                }

                // Check traditional parent classes with declared TypeVar variance
                // Pattern: class Child[T](Parent[T]) where Parent(Generic[X])
                // We need to find Parent's Generic params and their variances
                check_traditional_parent_variance(
                    &base_name,
                    pos,
                    typevar_declared,
                    &mut covariant_usage,
                    &mut contravariant_usage,
                );
            }
        }
    }

    // Phase 2: Scan methods for param usage positions
    let mut in_excluded_method = false;
    let mut has_setter_for_param = false;
    let mut has_public_attr_assignment = false;

    for body_line in &class.body_lines {
        // Track property setters
        if body_line.contains(".setter") {
            has_setter_for_param = true;
        }

        if body_line.starts_with("def ") {
            let method_name = body_line[4..]
                .split('(')
                .next()
                .unwrap_or("")
                .trim();
            let is_excluded = method_name == "__init__" || method_name == "__new__";
            in_excluded_method = is_excluded;

            if !is_excluded {
                analyze_method_signature(
                    body_line,
                    param,
                    &mut covariant_usage,
                    &mut contravariant_usage,
                );
            }
            continue;
        }

        // Check for public attribute assignment: self.attr = ... (not self._attr)
        if in_excluded_method
            && body_line.starts_with("self.")
            && body_line.contains('=')
            && !body_line.contains("==")
        {
            let attr_part = &body_line[5..];
            let attr_name = attr_part
                .split(|c: char| c == '=' || c == '.' || c == ' ')
                .next()
                .unwrap_or("")
                .trim();
            if !attr_name.starts_with('_') && !attr_name.is_empty() {
                has_public_attr_assignment = true;
            }
        }

        // Check class-level field annotations: x: T, x: Final[T]
        if !body_line.starts_with("def ")
            && !body_line.starts_with('@')
            && body_line.contains(':')
            && !body_line.starts_with("self.")
        {
            check_field_annotation(
                body_line,
                param,
                class,
                &mut covariant_usage,
                &mut contravariant_usage,
            );
        }
    }

    // Public mutable attribute (non-underscore) in __init__ → invariant
    if has_public_attr_assignment && !class.is_frozen_dataclass {
        return Variance::Invariant;
    }

    // Property getter + setter → invariant
    if has_setter_for_param && covariant_usage {
        return Variance::Invariant;
    }

    match (covariant_usage, contravariant_usage) {
        (true, true) => Variance::Invariant,
        (true, false) => Variance::Covariant,
        (false, true) => Variance::Contravariant,
        (false, false) => Variance::Invariant,
    }
}

/// Analyze a method signature for type param positions.
fn analyze_method_signature(
    line: &str,
    param: &str,
    covariant_usage: &mut bool,
    contravariant_usage: &mut bool,
) {
    // Check return type
    if let Some(arrow_pos) = line.find("->") {
        let return_part = &line[arrow_pos + 2..];
        let return_type = return_part.split(':').next().unwrap_or(return_part).trim();
        if contains_typevar_reference(return_type, param) {
            *covariant_usage = true;
        }
    }

    // Check parameter types (excluding self/cls)
    let Some(open) = line.find('(') else { return };
    let Some(close) = line.rfind(')') else { return };
    let params_text = &line[open + 1..close];

    for (pidx, p) in params_text.split(',').enumerate() {
        let p = p.trim();
        if pidx == 0 && (p.starts_with("self") || p.starts_with("cls")) {
            continue;
        }
        if let Some(colon) = p.find(':') {
            let ann = p[colon + 1..].split('=').next().unwrap_or("").trim();
            if contains_typevar_reference(ann, param) {
                *contravariant_usage = true;
            }
        }
    }
}

/// Check a class-level field annotation for variance implications.
fn check_field_annotation(
    line: &str,
    param: &str,
    class: &ClassForVariance,
    covariant_usage: &mut bool,
    contravariant_usage: &mut bool,
) {
    let Some((field_part, ann_part)) = line.split_once(':') else {
        return;
    };
    let field_name = field_part.trim();
    let ann = ann_part.split('=').next().unwrap_or(ann_part).trim();

    if field_name.is_empty()
        || !field_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
    {
        return;
    }

    if !contains_typevar_reference(ann, param) {
        return;
    }

    if ann.contains("Final") || class.is_frozen_dataclass {
        *covariant_usage = true;
    } else if class.is_dataclass {
        // Regular dataclass field = both positions
        *covariant_usage = true;
        *contravariant_usage = true;
    }
    // Non-dataclass fields without Final are handled by has_public_attr logic
}

/// Check if a traditional parent class constrains variance via its TypeVar declarations.
fn check_traditional_parent_variance(
    _base_name: &str,
    _pos: usize,
    typevar_declared: &HashMap<String, Variance>,
    covariant_usage: &mut bool,
    contravariant_usage: &mut bool,
) {
    // For traditional classes like Parent_Invariant(Generic[T]),
    // we look up the variance of the TypeVar used in that position.
    // Since we can't easily resolve the parent's Generic params from here,
    // we use a heuristic: check if the base_name contains a variance hint
    // or look up the parent class definition in the source.
    //
    // The typevar_declared map contains ALL TypeVars with their declared variance.
    // For the conformance test patterns like:
    //   class Parent_Invariant(Generic[T]): ...  (T is invariant)
    //   class Child[T](Parent_Invariant[T]): ...
    // We need to look up which TypeVar Parent_Invariant uses in position 0.
    //
    // This is handled by the collect phase which resolves parent Generic params.
    _ = (typevar_declared, covariant_usage, contravariant_usage);
}

/// Resolve traditional parent classes' Generic TypeVar variances.
fn resolve_parent_generic_variances(
    lines: &[&str],
    typevar_declared: &HashMap<String, Variance>,
) -> HashMap<String, Vec<Variance>> {
    let mut result = HashMap::new();

    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with("class ") || !trimmed.contains("Generic[") {
            continue;
        }

        let after_class = &trimmed[6..];
        let name_end = after_class
            .find(|c: char| c == '(' || c == '[' || c == ':')
            .unwrap_or(after_class.len());
        let name = after_class[..name_end].trim().to_owned();

        let params = extract_typevar_params_from_generic(trimmed);
        let variances: Vec<Variance> = params
            .iter()
            .map(|p| {
                typevar_declared
                    .get(p)
                    .copied()
                    .unwrap_or(Variance::Invariant)
            })
            .collect();

        let _ = result.insert(name, variances);
    }

    result
}

/// Main entry point: check variance-related assignment violations.
pub(super) fn check_variance_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = module.source.lines().collect();

    // Build map of TypeVars with infer_variance=true and declared variances
    let mut infer_variance_tvs: HashMap<String, Variance> = HashMap::new();
    let mut typevar_declared: HashMap<String, Variance> = HashMap::new();

    for tv in &module.typevar_calls {
        let var = if tv.is_covariant {
            Variance::Covariant
        } else if tv.is_contravariant {
            Variance::Contravariant
        } else {
            Variance::Invariant
        };
        let _ = typevar_declared.insert(tv.name.clone(), var);
        if tv.has_infer_variance {
            let _ = infer_variance_tvs.insert(tv.name.clone(), var);
        }
    }

    // Resolve traditional parent class variances
    let parent_variances = resolve_parent_generic_variances(&lines, &typevar_declared);

    // Collect all classes needing variance inference
    let classes = collect_classes_for_variance(&lines, &infer_variance_tvs);
    if classes.is_empty() {
        return;
    }

    // Phase 1: Infer variances (two passes for dependency resolution)
    let mut class_variances: HashMap<String, Vec<Variance>> = HashMap::new();

    // Merge parent variances into known set
    for (name, vars) in &parent_variances {
        let _ = class_variances.insert(name.clone(), vars.clone());
    }

    // First pass: infer without cross-class dependencies
    for class in &classes {
        let variances = infer_variances(class, &class_variances, &typevar_declared);
        let _ = class_variances.insert(class.name.clone(), variances);
    }

    // Second pass: re-infer with resolved dependencies
    for class in &classes {
        let variances = infer_variances(class, &class_variances, &typevar_declared);
        let _ = class_variances.insert(class.name.clone(), variances);
    }

    // Phase 2: Check assignments
    check_module_level_assignments(&lines, &class_variances, &module.source, &module.path, diagnostics);
    check_function_body_assignments(&lines, &class_variances, &module.source, &module.path, diagnostics);
}

/// Check module-level assignments like `v: Class[A] = Class[B]()`.
fn check_module_level_assignments(
    lines: &[&str],
    class_variances: &HashMap<String, Vec<Variance>>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();

        // Only module-level (no indentation)
        if line.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }

        // Must be an annotated assignment with generic types
        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let Some(eq_pos) = trimmed.find('=') else {
            continue;
        };
        if eq_pos < colon_pos {
            continue;
        }
        // Skip ==
        if trimmed.get(eq_pos + 1..eq_pos + 2) == Some("=") {
            continue;
        }

        let annotation = trimmed[colon_pos + 1..eq_pos].trim();
        let rhs = trimmed[eq_pos + 1..].split('#').next().unwrap_or("").trim();

        check_assignment_variance(
            annotation,
            rhs,
            class_variances,
            source,
            path,
            line_number,
            diagnostics,
        );
    }
}

/// Check assignments inside function bodies.
fn check_function_body_assignments(
    lines: &[&str],
    class_variances: &HashMap<String, Vec<Variance>>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Collect function parameter annotations
    let mut param_types: HashMap<String, (String, Vec<String>)> = HashMap::new();
    let mut in_function = false;
    let mut func_indent = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        if trimmed.starts_with("def ") {
            in_function = true;
            func_indent = indent;
            param_types.clear();

            // Parse function parameters (using bracket-aware splitting)
            if let Some(open) = trimmed.find('(') {
                if let Some(close) = trimmed.rfind(')') {
                    let params_text = &trimmed[open + 1..close];
                    let param_parts = split_top_level_params(params_text);
                    for p in &param_parts {
                        let p = p.trim();
                        if let Some(colon) = p.find(':') {
                            let pname = p[..colon].trim();
                            let ann = p[colon + 1..].split('=').next().unwrap_or("").trim();
                            if let Some((cls_name, args)) = parse_generic_annotation(ann) {
                                let _ = param_types
                                    .insert(pname.to_owned(), (cls_name, args));
                            }
                        }
                    }
                }
            }
            continue;
        }

        // Check if still inside the function
        if in_function && indent <= func_indent && !trimmed.is_empty() {
            in_function = false;
            param_types.clear();
        }

        if !in_function || param_types.is_empty() {
            continue;
        }

        // Look for annotated assignments: v: Class[A] = param_name
        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let Some(eq_pos) = trimmed.find('=') else {
            continue;
        };
        if eq_pos < colon_pos {
            continue;
        }
        if trimmed.get(eq_pos + 1..eq_pos + 2) == Some("=") {
            continue;
        }

        let annotation = trimmed[colon_pos + 1..eq_pos].trim();
        let rhs = trimmed[eq_pos + 1..].split('#').next().unwrap_or("").trim();

        // If RHS is a known parameter name with generic type
        eprintln!("  VARIANCE-DEBUG fn-body line {line_number}: rhs={rhs:?} annotation={annotation:?}");
        if let Some((rhs_class, rhs_args)) = param_types.get(rhs) {
            if let Some((lhs_class, lhs_args)) = parse_generic_annotation(annotation) {
                eprintln!("  VARIANCE-DEBUG match: lhs={lhs_class}[{lhs_args:?}] rhs={rhs_class}[{rhs_args:?}] has_var={}", class_variances.contains_key(&lhs_class));
                if lhs_class == *rhs_class {
                    if let Some(variances) = class_variances.get(&lhs_class) {
                        emit_variance_violations(
                            &lhs_class,
                            &lhs_args,
                            rhs_args,
                            variances,
                            source,
                            path,
                            line_number,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}

/// Check a single assignment for variance violations.
fn check_assignment_variance(
    annotation: &str,
    rhs: &str,
    class_variances: &HashMap<String, Vec<Variance>>,
    source: &str,
    path: &str,
    line_number: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((lhs_class, lhs_args)) = parse_generic_annotation(annotation) else {
        return;
    };

    // Extract RHS generic type from constructor call: Class[Type](...)
    let Some((rhs_class, rhs_args)) = extract_rhs_generic_type(rhs) else {
        return;
    };

    if lhs_class != rhs_class {
        return;
    }

    let Some(variances) = class_variances.get(&lhs_class) else {
        return;
    };

    emit_variance_violations(
        &lhs_class,
        &lhs_args,
        &rhs_args,
        variances,
        source,
        path,
        line_number,
        diagnostics,
    );
}

/// Split parameters at top-level commas (respecting brackets).
fn split_top_level_params(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(text[start..idx].to_owned());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let last = &text[start..];
    if !last.trim().is_empty() {
        parts.push(last.to_owned());
    }
    parts
}

/// Extract generic type from RHS like `ClassName[Type](args)`.
fn extract_rhs_generic_type(rhs: &str) -> Option<(String, Vec<String>)> {
    let bracket_pos = rhs.find('[')?;
    let class_name = rhs[..bracket_pos].trim().to_owned();

    // Find the matching ] for the type args
    let after_bracket = &rhs[bracket_pos + 1..];
    let mut depth = 0i32;
    let mut close_pos = None;
    for (idx, ch) in after_bracket.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    close_pos = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let close = close_pos?;
    let inner = &after_bracket[..close];

    let type_args = super::utils::split_top_level_type_args(inner);
    if type_args.is_empty() {
        return None;
    }

    Some((class_name, type_args))
}

/// Emit diagnostics for variance violations between LHS and RHS type args.
fn emit_variance_violations(
    class_name: &str,
    lhs_args: &[String],
    rhs_args: &[String],
    variances: &[Variance],
    source: &str,
    path: &str,
    line_number: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (idx, variance) in variances.iter().enumerate() {
        let Some(lhs_arg) = lhs_args.get(idx) else {
            continue;
        };
        let Some(rhs_arg) = rhs_args.get(idx) else {
            continue;
        };

        if lhs_arg == rhs_arg {
            continue;
        }

        let compatible = match variance {
            // Covariant: RHS must be subtype of LHS
            Variance::Covariant => is_numeric_subtype(rhs_arg, lhs_arg),
            // Contravariant: LHS must be subtype of RHS
            Variance::Contravariant => is_numeric_subtype(lhs_arg, rhs_arg),
            // Invariant: must be exact match
            Variance::Invariant => false,
        };

        if !compatible {
            eprintln!("  VARIANCE-EMIT line {line_number}: {class_name} idx={idx} lhs={lhs_arg} rhs={rhs_arg} var={variance:?}");
            let variance_label = match variance {
                Variance::Covariant => "covariant",
                Variance::Contravariant => "contravariant",
                Variance::Invariant => "invariant",
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Type `{class_name}[{rhs_arg}]` is not assignable to \
                     `{class_name}[{lhs_arg}]` (type parameter is {variance_label})"
                ),
                span: span_for_line(source, line_number),
                path: path.to_owned(),
                help: Some(format!(
                    "{variance_label} type parameter requires {requirement}",
                    requirement = match variance {
                        Variance::Covariant => "subtype relationship (e.g. int → float)",
                        Variance::Contravariant =>
                            "supertype relationship (e.g. float → int)",
                        Variance::Invariant => "exact type match",
                    }
                )),
                note: Some(
                    "PEP 695: variance is inferred from type parameter usage positions"
                        .to_owned(),
                ),
            });
        }
    }
}
