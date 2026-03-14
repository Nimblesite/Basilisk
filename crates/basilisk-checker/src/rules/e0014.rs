//! BSK-E0014: Assignment type incompatibility (literal mismatches).
//!
//! Detects annotated module-level variables where the declared type and the
//! literal kind of the right-hand side are clearly incompatible, for example:
//!
//! ```python
//! count: int = "hello"   # str literal assigned to int annotation → E0014
//! label: str = 42        # int literal assigned to str annotation → E0014
//! flag:  bool = "yes"    # str literal assigned to bool annotation → E0014
//! ratio: float = "1.5"   # str literal assigned to float annotation → E0014
//! ```
//!
//! The check is performed by extracting the annotation text from the source
//! around the variable's name span and comparing it against the RHS kind.

use std::collections::HashMap;

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::{InferredType, LiteralValue};
use basilisk_resolver::{ResolvedModule, RhsKind, Span, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0014",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0014",
};

/// Emits BSK-E0014 for annotated module variables whose annotation and literal
/// RHS are obviously incompatible.
pub(crate) struct AssignmentTypeMismatch;

impl Rule for AssignmentTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let empty_params = std::collections::HashMap::new();
        check_vars(
            &module.module_vars,
            &module.source,
            &module.path,
            diagnostics,
            &empty_params,
        );
        check_local_vars(module, diagnostics);
        check_tuple_reassignments(module, diagnostics);
        check_dataclass_attr_assignments(module, diagnostics);
    }
}

/// Check a slice of annotated variables for type mismatches.
///
/// `param_types` maps parameter names to their declared annotation types.
/// When the RHS of an annotated local variable is a simple name reference
/// that matches a parameter, the parameter's type is used for assignability
/// checking instead of the generic `Unknown` fallback.
fn check_vars(
    vars: &[VariableInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    param_types: &std::collections::HashMap<String, InferredType>,
) {
    vars.iter()
        .filter(|var| var.has_annotation && var.rhs_span.is_some())
        .filter_map(|var| {
            let annotation_text = extract_annotation(source, var.name_span)?;
            let declared_type = InferredType::from_annotation(annotation_text);

            // When the declared type is a Literal, try to infer the RHS as a
            // literal value so we can compare values, not just kinds.
            let mut inferred_type = infer_with_literal_value(var, source, &declared_type);

            // When the inferred type is Unknown and the RHS text is a parameter
            // name, use the parameter's declared type instead.
            if matches!(inferred_type, InferredType::Unknown) {
                if let Some(rhs_span) = var.rhs_span {
                    if let Some(rhs_text) = slice_span(source, rhs_span) {
                        let rhs_name = rhs_text.trim();
                        if let Some(param_type) = param_types.get(rhs_name) {
                            inferred_type = param_type.clone();
                        }
                    }
                }
            }

            if inferred_type.is_assignable_to(&declared_type) {
                None
            } else {
                Some((
                    var,
                    annotation_text.to_owned(),
                    inferred_type,
                    declared_type,
                ))
            }
        })
        .for_each(|(var, annotation, inferred, declared)| {
            diagnostics.push(make_diagnostic(
                var,
                &annotation,
                &inferred,
                &declared,
                path,
            ));
        });
}

/// Infer the RHS type, upgrading to a `Literal[value]` when the declared type
/// is itself a `Literal` and we can extract the actual value from source text.
fn infer_with_literal_value(
    var: &VariableInfo,
    source: &str,
    declared: &InferredType,
) -> InferredType {
    let base = infer_rhs(&var.rhs_kind);

    // Only attempt value-level inference when the target is a Literal type
    let is_literal_target = matches!(declared, InferredType::Literal(_) | InferredType::Union(_));
    if !is_literal_target {
        return base;
    }

    // Extract the RHS source text
    let Some(rhs_span) = var.rhs_span else {
        return base;
    };
    let rhs_text = match slice_span(source, rhs_span) {
        Some(text) => text.trim(),
        None => return base,
    };

    // Try to parse a literal value from the source text
    match var.rhs_kind {
        RhsKind::IntLiteral => parse_int_literal(rhs_text).unwrap_or(base),
        RhsKind::StrLiteral => parse_str_literal(rhs_text).unwrap_or(base),
        RhsKind::BoolLiteral => parse_bool_literal(rhs_text).unwrap_or(base),
        RhsKind::FloatLiteral => parse_float_literal(rhs_text).unwrap_or(base),
        RhsKind::BytesLiteral => parse_bytes_literal(rhs_text).unwrap_or(base),
        _ => base,
    }
}

/// Parse an integer literal from source text into `Literal[value]`.
fn parse_int_literal(text: &str) -> Option<InferredType> {
    let text = text.trim().replace('_', "");
    // Handle hex, octal, binary
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let val = i64::from_str_radix(hex, 16).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    if let Some(oct) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        let val = i64::from_str_radix(oct, 8).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        let val = i64::from_str_radix(bin, 2).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    // Handle negative
    if let Some(neg) = text.strip_prefix('-') {
        let val = neg.trim().parse::<i64>().ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(-val)));
    }
    let val = text.parse::<i64>().ok()?;
    Some(InferredType::Literal(LiteralValue::Int(val)))
}

/// Parse a string literal from source text into `Literal[value]`.
fn parse_str_literal(text: &str) -> Option<InferredType> {
    let text = text.trim();
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        let content = text.get(1..text.len().saturating_sub(1))?;
        return Some(InferredType::Literal(LiteralValue::Str(content.to_owned())));
    }
    None
}

/// Parse a boolean literal from source text into `Literal[value]`.
fn parse_bool_literal(text: &str) -> Option<InferredType> {
    match text.trim() {
        "True" => Some(InferredType::Literal(LiteralValue::Bool(true))),
        "False" => Some(InferredType::Literal(LiteralValue::Bool(false))),
        _ => None,
    }
}

/// Parse a float literal from source text into `Literal[value]`.
fn parse_float_literal(text: &str) -> Option<InferredType> {
    let text = text.trim().replace('_', "");
    let val = text.parse::<f64>().ok()?;
    Some(InferredType::Literal(LiteralValue::Float(val)))
}

/// Parse a bytes literal from source text into `Literal[value]`.
fn parse_bytes_literal(text: &str) -> Option<InferredType> {
    let text = text.trim();
    if (text.starts_with("b\"") || text.starts_with("b'"))
        && (text.ends_with('"') || text.ends_with('\''))
    {
        let content = text.get(2..text.len().saturating_sub(1))?;
        return Some(InferredType::Literal(LiteralValue::Bytes(
            content.as_bytes().to_vec(),
        )));
    }
    None
}

/// Check local variables in function bodies for type mismatches.
///
/// Builds a map of parameter name to declared type for each function so that
/// assignments like `x: Literal[False] = a` (where `a: Literal[0]`) can be
/// checked for Literal-level incompatibility.
fn check_local_vars(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    for func in &module.functions {
        let param_types = build_param_type_map(&func.parameters, source);
        check_vars(
            &func.local_vars,
            source,
            &module.path,
            diagnostics,
            &param_types,
        );
    }
}

/// Build a map from parameter name to its declared `InferredType` by reading
/// the annotation text from source spans.
fn build_param_type_map(
    params: &[basilisk_resolver::ParameterInfo],
    source: &str,
) -> std::collections::HashMap<String, InferredType> {
    let mut map = std::collections::HashMap::new();
    for param in params {
        if !param.has_annotation {
            continue;
        }
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let inferred = InferredType::from_annotation(ann_text.trim());
        let _ = map.insert(param.name.clone(), inferred);
    }
    map
}

/// Create diagnostic for inference-based type mismatch
fn make_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    inferred: &InferredType,
    declared: &InferredType,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Type mismatch: `{}` is annotated `{annotation}` ({}) but assigned {}",
            var.name, declared, inferred
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Either change the annotation to match the value, or change the value to `{annotation}`"
        )),
        note: Some(
            "Basilisk requires the inferred type to be assignable to the declared type".to_owned(),
        ),
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
fn extract_annotation(source: &str, name_span: Span) -> Option<&str> {
    // Find the byte offset of the start of the line containing the name.
    let start = usize::try_from(name_span.start).ok()?;
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
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

/// Check re-assignments to tuple-annotated variables against the tuple literal RHS.
///
/// For example, `t1: tuple[int]` declared, then `t1 = (1, 2)` assigned — error because
/// `(1, 2)` has 2 elements but `tuple[int]` requires exactly 1.
fn check_tuple_reassignments(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Build map: var name → annotation text, for vars annotated with tuple types.
    let mut tuple_annotations: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_trimmed = ann_text.trim();
        if is_tuple_annotation(ann_trimmed) {
            let _ = tuple_annotations.insert(var.name.as_str(), ann_trimmed);
        }
    }

    if tuple_annotations.is_empty() {
        return;
    }

    // Check unannotated re-assignments to tuple-annotated variables.
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(&ann_text) = tuple_annotations.get(var.name.as_str()) else {
            continue;
        };
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_trimmed = rhs_text.trim();

        if !is_tuple_literal(rhs_trimmed) {
            continue;
        }

        if let Some(msg) = check_tuple_literal_mismatch(rhs_trimmed, ann_text) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Type mismatch: `{}` is annotated `{ann_text}` but assigned {msg}",
                    var.name
                ),
                span: var.name_span,
                path: path.to_owned(),
                help: Some("Ensure the tuple literal matches the annotated tuple type".to_owned()),
                note: Some(
                    "Basilisk checks that tuple literals are compatible with the declared tuple type"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Returns `true` if the annotation is a simple tuple type (no starred components).
///
/// Skips complex types like `tuple[int, *tuple[str, ...]]` that require variadic analysis.
fn is_tuple_annotation(ann: &str) -> bool {
    if !ann.starts_with("tuple[") || !ann.ends_with(']') {
        return false;
    }
    // Skip annotations with starred components (TypeVarTuple unpacks)
    match ann.get("tuple[".len()..ann.len().saturating_sub(1)) {
        Some(inner) => !inner.contains('*'),
        None => false,
    }
}

/// Returns `true` if the source text looks like a tuple literal `(...)`.
fn is_tuple_literal(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

/// Returns `Some(description)` when the tuple literal is incompatible with the annotation.
fn check_tuple_literal_mismatch(rhs: &str, ann: &str) -> Option<String> {
    let inner_ann = ann.strip_prefix("tuple[")?.strip_suffix(']')?;

    // Inner content of the tuple literal `(...)`.
    let rhs_inner = rhs.strip_prefix('(')?.strip_suffix(')')?;
    let rhs_elems = split_tuple_literal_elems(rhs_inner);

    // Homogeneous variable-length tuple: `tuple[T, ...]`
    if let Some(elem_type) = inner_ann.strip_suffix(", ...") {
        let elem_type = elem_type.trim();
        for elem in &rhs_elems {
            let elem = elem.trim();
            if !elem.is_empty() && !literal_elem_matches(elem, elem_type) {
                return Some(format!(
                    "a tuple containing `{elem}` (incompatible with `{elem_type}`)"
                ));
            }
        }
        return None;
    }

    // Empty tuple: `tuple[()]`
    if inner_ann.trim() == "()" {
        if !(rhs_elems.is_empty()
            || rhs_elems.len() == 1 && rhs_elems.first().is_some_and(|elem| elem.trim().is_empty()))
        {
            return Some(format!(
                "a tuple with {} element(s) (expected empty tuple)",
                rhs_elems.len()
            ));
        }
        return None;
    }

    // Fixed-length tuple: split annotation into element types.
    let ann_elems = split_type_list(inner_ann);

    // Count mismatch.
    if rhs_elems.len() != ann_elems.len() {
        return Some(format!(
            "a {}-element tuple (expected {} element(s))",
            rhs_elems.len(),
            ann_elems.len()
        ));
    }

    // Element type mismatches.
    for (idx, (rhs_elem, ann_elem)) in rhs_elems.iter().zip(ann_elems.iter()).enumerate() {
        let rhs_e = rhs_elem.trim();
        let ann_e = ann_elem.trim();
        if !rhs_e.is_empty() && !literal_elem_matches(rhs_e, ann_e) {
            return Some(format!(
                "a tuple with element {idx} `{rhs_e}` (expected type `{ann_e}`)"
            ));
        }
    }

    None
}

/// Split the inner content of a tuple literal by top-level commas.
/// Handles trailing commas: `1,` → `["1"]`, `1, 2` → `["1", "2"]`.
fn split_tuple_literal_elems(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    parts.push(part.trim());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Split a comma-separated type list respecting bracket nesting.
fn split_type_list(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    let part = part.trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Returns `true` if a literal element (source text) is compatible with `expected_type`.
fn literal_elem_matches(elem: &str, expected: &str) -> bool {
    let expected_lower = expected.to_ascii_lowercase();
    let expected_base = expected_lower
        .split('[')
        .next()
        .unwrap_or(expected_lower.as_str())
        .trim();

    if expected_base == "any" || expected_base == "object" {
        return true;
    }

    let is_int_lit = elem
        .chars()
        .all(|c| c.is_ascii_digit() || c == '_' || c == 'x' || c == 'o' || c == 'b')
        && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_str_lit = (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''));
    let is_float_lit =
        elem.contains('.') && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_bytes_lit = (elem.starts_with("b\"") || elem.starts_with("b'"))
        && (elem.ends_with('"') || elem.ends_with('\''));
    let is_bool_lit = elem == "True" || elem == "False";
    let is_none_lit = elem == "None";

    match expected_base {
        "int" => is_int_lit || is_bool_lit,
        "float" | "complex" => is_float_lit || is_int_lit || is_bool_lit,
        "str" => is_str_lit,
        "bytes" => is_bytes_lit,
        "bool" => is_bool_lit,
        "none" => is_none_lit,
        _ => true, // Unknown types: don't flag
    }
}

// ---------------------------------------------------------------------------
// Dataclass attribute assignment type mismatch
// ---------------------------------------------------------------------------

/// Returns `Some(description)` when the annotation text and RHS kind are
/// clearly incompatible; `None` when the pairing is acceptable or unknown.
fn annotation_rhs_mismatch_simple(annotation: &str, rhs: &RhsKind) -> Option<&'static str> {
    // Normalise: strip generic parameters and whitespace, lower-case.
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        _ => None,
    }
}

/// Checks module-level attribute assignments (`instance.field = value`) against
/// the declared field types of `dataclass`/`dataclass_transform` classes.
fn check_dataclass_attr_assignments(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    if module.module_attr_assignments.is_empty() {
        return;
    }

    let transform_classes = super::guards::collect_transform_classes(module);

    // Build a map: class_name -> { field_name -> annotation_text }
    let mut class_field_types: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for cls in &module.classes {
        let is_dc_like = cls.is_dataclass || transform_classes.contains_key(cls.name.as_str());
        if !is_dc_like {
            continue;
        }
        let mut fields = HashMap::new();
        for attr in &cls.attributes {
            if let Some(ann_span) = attr.annotation_span {
                if let Some(ann_text) = slice_span(&module.source, ann_span) {
                    let _ = fields.insert(attr.name.as_str(), ann_text.trim());
                }
            }
        }
        let _ = class_field_types.insert(cls.name.as_str(), fields);
    }

    if class_field_types.is_empty() {
        return;
    }

    // Build a map: variable_name -> class_name (for instances of DC-like classes)
    let source = &module.source;
    let instance_class: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter_map(|var| {
            let rhs_span = var.rhs_span?;
            let rhs_text = slice_span(source, rhs_span)?;
            let callee = rhs_text.split(['(', '[']).next()?.trim();
            let callee = callee.rsplit('.').next().unwrap_or(callee);
            if class_field_types.contains_key(callee) {
                Some((var.name.as_str(), callee))
            } else {
                None
            }
        })
        .collect();

    if instance_class.is_empty() {
        return;
    }

    for assign in &module.module_attr_assignments {
        let Some(&class_name) = instance_class.get(assign.object_name.as_str()) else {
            continue;
        };
        let Some(fields) = class_field_types.get(class_name) else {
            continue;
        };
        let Some(&field_type) = fields.get(assign.attr_name.as_str()) else {
            continue;
        };

        // Extract the RHS literal kind from the source line
        let rhs_kind = extract_rhs_kind_from_assign(source, assign.target_span);
        if let Some(kind) = rhs_kind {
            if let Some(rhs_description) = annotation_rhs_mismatch_simple(field_type, &kind) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Type mismatch: `{}.{}` is typed `{field_type}` but assigned {rhs_description}",
                        assign.object_name, assign.attr_name
                    ),
                    span: assign.target_span,
                    path: module.path.clone(),
                    help: Some(format!(
                        "Field `{}` of `{class_name}` expects `{field_type}`",
                        assign.attr_name
                    )),
                    note: Some(
                        "Basilisk requires attribute assignments to be compatible with the declared field type"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Extracts the RHS literal kind from a module-level attribute assignment line.
///
/// Given the target span of `obj.attr` in `obj.attr = value`, finds the `= value`
/// portion and determines the literal kind.
fn extract_rhs_kind_from_assign(source: &str, target_span: Span) -> Option<RhsKind> {
    let target_end = target_span.end_usize();
    let line_end = source
        .get(target_end..)?
        .find('\n')
        .map_or(source.len(), |pos| target_end + pos);
    let after_target = source.get(target_end..line_end)?;

    // Find `=` after the target
    let eq_pos = after_target.find('=')?;
    let rhs = after_target.get(eq_pos + 1..)?.trim();

    classify_literal(rhs)
}

/// Classifies a simple literal token into a `RhsKind`.
fn classify_literal(text: &str) -> Option<RhsKind> {
    if text.is_empty() {
        return None;
    }

    // Integer literal: starts with digit, no dot
    if text.bytes().next()?.is_ascii_digit() {
        if text.contains('.') {
            return Some(RhsKind::FloatLiteral);
        }
        return Some(RhsKind::IntLiteral);
    }

    // String literal
    if text.starts_with('"')
        || text.starts_with('\'')
        || text.starts_with("f\"")
        || text.starts_with("f'")
    {
        return Some(RhsKind::StrLiteral);
    }

    // Bytes literal
    if text.starts_with("b\"") || text.starts_with("b'") {
        return Some(RhsKind::BytesLiteral);
    }

    // None
    if text.starts_with("None") {
        return Some(RhsKind::NoneValue);
    }

    // Negative numbers
    if text.starts_with('-') {
        return classify_literal(text.get(1..)?.trim_start());
    }

    None
}
