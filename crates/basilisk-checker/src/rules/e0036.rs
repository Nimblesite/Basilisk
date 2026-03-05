//! BSK-E0036: `ClassVar` used in an invalid context.
//!
//! PEP 526 and the typing spec restrict `ClassVar[T]` to:
//!
//! - Annotations of class body attributes (class variables)
//!
//! Using `ClassVar` outside a class body (in function parameters, return types,
//! local variable annotations, or module-level variable annotations) is an error.
//! Additionally, nesting `ClassVar` inside another type constructor (e.g.
//! `Final[ClassVar[int]]` or `list[ClassVar[int]]`) is forbidden.
//!
//! Note: `Annotated[ClassVar[T], ...]` is a valid exception.
//!
//! This rule also validates `ClassVar` argument correctness:
//! - `ClassVar` accepts at most one argument
//! - The argument must be a valid type (not a literal or runtime variable)
//! - The argument must not contain `TypeVar`, `ParamSpec`, or `TypeVarTuple`
//!
//! Additionally, `ClassVar` attributes cannot be assigned via instances.
//!
//! ```python
//! class MyClass:
//!     bad9: Final[ClassVar[int]] = 3     # E0036 — ClassVar cannot be nested
//!     bad10: list[ClassVar[int]] = []    # E0036 — ClassVar cannot be nested
//!
//!     def method1(self, a: ClassVar[int]):   # E0036 — ClassVar not allowed here
//!         x: ClassVar[str] = ""              # E0036 — ClassVar not allowed here
//!         self.xx: ClassVar[str] = ""        # E0036 — ClassVar not allowed here
//!
//!     def method2(self) -> ClassVar[int]:    # E0036 — ClassVar not allowed here
//!         ...
//!
//! bad11: ClassVar[int] = 3              # E0036 — ClassVar not allowed at module level
//! bad12: TypeAlias = ClassVar[str]      # E0036 — ClassVar not allowed here
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0036",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0036",
};

/// Returns the text slice for a span within the source.
fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

/// Returns `true` when the annotation text contains `ClassVar[` at all —
/// used for contexts where ANY `ClassVar` usage is invalid (function params,
/// return types, module-level annotations).
fn has_classvar(ann: &str) -> bool {
    ann.contains("ClassVar[") || ann.contains("ClassVar ")
}

/// Returns `true` when `ClassVar` or an alias like `CV` appears as a bare
/// name or subscript in an annotation string.
fn has_classvar_or_alias(ann: &str) -> bool {
    has_classvar(ann) || ann.contains("CV[") || ann == "ClassVar" || ann == "CV"
}

/// Returns `true` when the annotation text contains `ClassVar` nested inside
/// another type constructor.  `Annotated[ClassVar[...], ...]` is excluded
/// because that is a valid usage per the typing spec.
///
/// Pattern: `[ClassVar[` appears in the annotation (meaning something wraps it)
/// AND the annotation does not begin with `Annotated[`.
fn has_nested_classvar(ann: &str) -> bool {
    ann.contains("[ClassVar[") && !ann.starts_with("Annotated[")
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some("`ClassVar` is only valid as a class body attribute annotation".to_owned()),
        note: Some(
            "PEP 526: `ClassVar` cannot appear in function signatures, local variables, \
             or module-level annotations, and cannot be nested inside another type"
                .to_owned(),
        ),
    }
}

/// Extract the content between the outer `[...]` of a `ClassVar[...]` or `CV[...]`
/// annotation text.  Returns `None` when there is no subscript.
fn extract_classvar_inner(ann: &str) -> Option<&str> {
    // Find the start: "ClassVar[" or "CV["
    let prefix_len = if ann.starts_with("ClassVar[") {
        "ClassVar[".len()
    } else if ann.starts_with("CV[") {
        "CV[".len()
    } else if ann.starts_with("Annotated[ClassVar[") {
        // Skip Annotated wrapper — valid per spec
        return None;
    } else {
        return None;
    };

    // Find the matching closing bracket by counting nesting
    let bytes = ann.as_bytes();
    let mut depth: u32 = 1;
    let mut end_idx = None;
    for (idx, &byte) in bytes.iter().enumerate().skip(prefix_len) {
        match byte {
            b'[' => depth = depth.saturating_add(1),
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end_idx = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.map(|end| &ann[prefix_len..end])
}

/// Count the number of top-level comma-separated arguments in a bracket body.
fn count_top_level_args(inner: &str) -> usize {
    if inner.trim().is_empty() {
        return 0;
    }
    let mut depth: u32 = 0;
    let mut count: usize = 1;
    for byte in inner.as_bytes() {
        match byte {
            b'[' | b'(' => depth = depth.saturating_add(1),
            b']' | b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => count = count.saturating_add(1),
            _ => {}
        }
    }
    count
}

/// Returns `true` when the argument text looks like a numeric literal (e.g. `3`, `3.14`).
fn is_numeric_literal(arg: &str) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A numeric literal: all digits, optionally with a single dot
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+')
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

/// Known built-in type names that are valid as `ClassVar` arguments even though
/// they start with lowercase (e.g. `int`, `str`, `float`, `bool`, `bytes`, `list`,
/// `dict`, `set`, `tuple`, `type`, `object`, `complex`, `range`, `slice`,
/// `frozenset`, `bytearray`, `memoryview`).
const LOWERCASE_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "list",
    "dict",
    "set",
    "tuple",
    "type",
    "object",
    "complex",
    "range",
    "slice",
    "frozenset",
    "bytearray",
    "memoryview",
    "property",
    "staticmethod",
    "classmethod",
    "super",
];

/// Returns `true` when the argument text looks like a runtime variable reference
/// (a simple identifier that is not a known type name).
///
/// A bare identifier that starts with a lowercase letter and is NOT one of the
/// known built-in types is considered a runtime variable.
fn is_runtime_variable(arg: &str, module_var_names: &[String]) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must be a simple identifier (no brackets, dots, etc.)
    if !trimmed.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return false;
    }
    // Check if it's a known module-level variable (runtime value)
    if module_var_names.iter().any(|name| name == trimmed) {
        return true;
    }
    // A bare lowercase identifier that is not a known type is likely a runtime variable
    let first_char = trimmed.chars().next();
    if first_char.is_some_and(|ch| ch.is_ascii_lowercase()) {
        return !LOWERCASE_TYPE_NAMES.contains(&trimmed);
    }
    false
}

/// Check if an annotation's `ClassVar` argument contains any of the given type
/// parameter names (`TypeVar`, `ParamSpec`, `TypeVarTuple` names).
fn contains_type_param(
    ann_inner: &str,
    type_param_names: &[(String, TypeParamKind)],
) -> Option<TypeParamKind> {
    for (name, kind) in type_param_names {
        // Check for the name appearing as a standalone word or as part of a subscript
        // e.g. `T` in `list[T]`, `P` in `Callable[P, Any]`
        if contains_word(ann_inner, name) {
            return Some(*kind);
        }
    }
    None
}

/// Check if `text` contains `word` as a standalone identifier (not part of a larger name).
fn contains_word(text: &str, word: &str) -> bool {
    let word_bytes = word.as_bytes();
    let text_bytes = text.as_bytes();
    let word_len = word_bytes.len();

    if word_len > text_bytes.len() {
        return false;
    }

    for start_idx in 0..=text_bytes.len().saturating_sub(word_len) {
        if &text_bytes[start_idx..start_idx + word_len] == word_bytes {
            // Check that the character before (if any) is not alphanumeric or underscore
            let before_ok =
                start_idx == 0 || !is_ident_char(text_bytes[start_idx.saturating_sub(1)]);
            // Check that the character after (if any) is not alphanumeric or underscore
            let after_ok = start_idx + word_len >= text_bytes.len()
                || !is_ident_char(text_bytes[start_idx + word_len]);
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if the byte is an ASCII alphanumeric or underscore character.
fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Classification of a type parameter for error messaging.
#[derive(Debug, Clone, Copy)]
enum TypeParamKind {
    TypeVar,
    ParamSpec,
    TypeVarTuple,
}

/// Scan source text for `self.<name>: ClassVar` or `self.<name>: CV` patterns
/// and return the spans and names of violations.
fn find_self_classvar_annotations(source: &str) -> Vec<(String, Span)> {
    let mut results = Vec::new();
    let bytes = source.as_bytes();
    let source_len = bytes.len();
    let self_dot = b"self.";

    let mut idx = 0;
    while idx + self_dot.len() < source_len {
        // Find "self."
        if &bytes[idx..idx + self_dot.len()] != self_dot {
            idx += 1;
            continue;
        }

        // Check that "self." is preceded by whitespace/newline/start (not part of a larger name)
        if idx > 0 && is_ident_char(bytes[idx - 1]) {
            idx += 1;
            continue;
        }

        let attr_start = idx + self_dot.len();

        // Collect the attribute name
        let mut attr_end = attr_start;
        while attr_end < source_len && is_ident_char(bytes[attr_end]) {
            attr_end += 1;
        }
        if attr_end == attr_start {
            idx += 1;
            continue;
        }

        let attr_name = if let Ok(name) = std::str::from_utf8(&bytes[attr_start..attr_end]) {
            name.to_owned()
        } else {
            idx += 1;
            continue;
        };

        // Skip whitespace after the attribute name
        let mut colon_idx = attr_end;
        while colon_idx < source_len && bytes[colon_idx] == b' ' {
            colon_idx += 1;
        }

        // Check for ":"
        if colon_idx >= source_len || bytes[colon_idx] != b':' {
            idx = attr_end;
            continue;
        }

        // Skip whitespace after ":"
        let mut ann_start = colon_idx + 1;
        while ann_start < source_len && bytes[ann_start] == b' ' {
            ann_start += 1;
        }

        // Check if annotation starts with "ClassVar" or "CV"
        let has_cv =
            if ann_start + 8 <= source_len && &bytes[ann_start..ann_start + 8] == b"ClassVar" {
                true
            } else {
                ann_start + 2 <= source_len
                    && &bytes[ann_start..ann_start + 2] == b"CV"
                    && (ann_start + 2 >= source_len
                        || bytes[ann_start + 2] == b'['
                        || bytes[ann_start + 2] == b' ')
            };

        if has_cv {
            let target_start = idx;
            #[allow(clippy::cast_possible_truncation)]
            let span = Span {
                start: target_start as u32,
                end: attr_end as u32,
            };
            results.push((attr_name, span));
        }

        idx = attr_end;
    }

    results
}

/// Emits BSK-E0036 for `ClassVar` used in an invalid context.
pub(crate) struct ClassVarInvalidContext;

impl Rule for ClassVarInvalidContext {
    #[allow(clippy::too_many_lines)]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect TypeVar/ParamSpec/TypeVarTuple names for ClassVar argument validation
        let type_param_names: Vec<(String, TypeParamKind)> = module
            .typevar_calls
            .iter()
            .map(|tc| {
                let kind = if tc.is_paramspec {
                    TypeParamKind::ParamSpec
                } else if tc.is_typevartuple {
                    TypeParamKind::TypeVarTuple
                } else {
                    TypeParamKind::TypeVar
                };
                (tc.name.clone(), kind)
            })
            .collect();

        // Collect module-level variable names for runtime variable detection
        let module_var_names: Vec<String> = module
            .module_vars
            .iter()
            .filter(|var| {
                // Exclude TypeVar/ParamSpec/TypeVarTuple assignments
                !type_param_names.iter().any(|(name, _)| name == &var.name)
            })
            .map(|var| var.name.clone())
            .collect();

        // --- Class attributes: detect nested ClassVar and validate arguments ---
        for cls in &module.classes {
            // Also collect generic params from the class itself
            let class_type_params: Vec<(String, TypeParamKind)> = cls
                .generic_params
                .iter()
                .map(|gp| {
                    let kind = if gp.is_typevartuple {
                        TypeParamKind::TypeVarTuple
                    } else {
                        TypeParamKind::TypeVar
                    };
                    (gp.name.clone(), kind)
                })
                .collect();

            // Merge module-level and class-level type params
            let all_type_params: Vec<(String, TypeParamKind)> = type_param_names
                .iter()
                .chain(class_type_params.iter())
                .cloned()
                .collect();

            for attr in &cls.attributes {
                let Some(ann) = span_text(source, attr.annotation_span) else {
                    continue;
                };

                // Check for nested ClassVar (e.g. Final[ClassVar[int]])
                if has_nested_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` cannot be nested inside another type in attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }

                // Validate ClassVar arguments and type mismatch
                if let Some(inner) = extract_classvar_inner(ann) {
                    check_classvar_args(
                        inner,
                        &attr.name,
                        attr.name_span,
                        path,
                        &all_type_params,
                        &module_var_names,
                        diagnostics,
                    );

                    // Check for type mismatch between ClassVar type and RHS value
                    if let Some(rhs_text) = span_text(source, attr.rhs_span) {
                        check_classvar_type_mismatch(
                            inner,
                            rhs_text,
                            &attr.name,
                            attr.name_span,
                            path,
                            diagnostics,
                        );
                    }
                }
            }
        }

        // --- Function parameters: ClassVar not allowed ---
        for func in &module.functions {
            for param in func
                .parameters
                .iter()
                .chain(func.vararg.iter())
                .chain(func.kwarg.iter())
            {
                let Some(ann) = span_text(source, param.annotation_span) else {
                    continue;
                };
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in parameter annotation for `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }

            // --- Function return type: ClassVar not allowed ---
            if let Some(ret_ann) = span_text(source, func.return_annotation_span) {
                if has_classvar(ret_ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in the return annotation of `{}`",
                            func.name
                        ),
                        func.name_span,
                        path,
                    ));
                }
            }

            // --- Local variables: ClassVar not allowed ---
            for var in &func.local_vars {
                let Some(ann) = span_text(source, var.annotation_span) else {
                    continue;
                };
                if has_classvar_or_alias(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in local variable annotation for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Self-attribute ClassVar annotations: scan source text ---
        // e.g. `self.xx: ClassVar[str] = ""`
        // These are not captured in local_vars because the target is an Attribute node.
        let self_classvar_violations = find_self_classvar_annotations(source);
        for (attr_name, span) in &self_classvar_violations {
            // Only flag self.xxx ClassVar inside methods (verify by checking the span
            // falls within a function that has a class_name)
            let in_method = module.functions.iter().any(|func| {
                func.class_name.is_some()
                    && func.def_span.start <= span.start
                    // Use the next function/class boundary or end of source as upper bound
                    && span.start > func.name_span.start
            });
            if in_method {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` is not allowed in self-attribute annotation for `{attr_name}`",
                    ),
                    *span,
                    path,
                ));
            }
        }

        // --- Module-level variables: ClassVar not allowed ---
        for var in &module.module_vars {
            // Check annotation span (for `bad11: ClassVar[int] = 3`)
            if let Some(ann) = span_text(source, var.annotation_span) {
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in module-level annotation for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                    // Don't double-report for the same variable
                    continue;
                }
            }
            // Check RHS span (for `bad12: TypeAlias = ClassVar[str]`)
            if let Some(rhs) = span_text(source, var.rhs_span) {
                if has_classvar(rhs) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in right-hand side of module-level \
                             assignment for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Instance-level assignment to ClassVar attributes ---
        // e.g. `enterprise_d.stats = {}` where `stats` is ClassVar in the class
        check_instance_classvar_assignments(module, diagnostics);

        // --- Protocol ClassVar conformance ---
        // e.g. `a: ProtoA = ProtoAImpl()` where ProtoA requires ClassVar attrs
        check_protocol_classvar_conformance(module, diagnostics);
    }
}

/// Validate `ClassVar` arguments for correctness.
fn check_classvar_args(
    inner: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    type_param_names: &[(String, TypeParamKind)],
    module_var_names: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_count = count_top_level_args(inner);

    // Too many arguments (ClassVar accepts at most 1)
    if arg_count > 1 {
        diagnostics.push(make_diagnostic(
            format!(
                "`ClassVar` accepts at most one type argument, but `{attr_name}` has {arg_count}",
            ),
            name_span,
            path,
        ));
        return;
    }

    let trimmed = inner.trim();

    // Check for numeric literal argument (e.g. ClassVar[3])
    if is_numeric_literal(trimmed) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is not a valid type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for runtime variable argument (e.g. ClassVar[var])
    if is_runtime_variable(trimmed, module_var_names) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is a runtime variable, not a type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for TypeVar/ParamSpec/TypeVarTuple in ClassVar argument
    if let Some(kind) = contains_type_param(trimmed, type_param_names) {
        let kind_name = match kind {
            TypeParamKind::TypeVar => "TypeVar",
            TypeParamKind::ParamSpec => "ParamSpec",
            TypeParamKind::TypeVarTuple => "TypeVarTuple",
        };
        diagnostics.push(make_diagnostic(
            format!("`ClassVar` parameter for `{attr_name}` cannot contain {kind_name}",),
            name_span,
            path,
        ));
    }
}

/// Check for type mismatch between a `ClassVar` annotation's inner type and the
/// RHS value.  For example, `ClassVar[list[str]] = {}` is a mismatch because `{}`
/// is a dict literal but the annotation expects a list.
fn check_classvar_type_mismatch(
    inner: &str,
    rhs_text: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed_inner = inner.trim();
    let trimmed_rhs = rhs_text.trim();

    if trimmed_rhs.is_empty() || trimmed_inner.is_empty() {
        return;
    }

    // Detect dict literal `{}` or `{...}` assigned to a list/set/tuple type
    let rhs_is_dict = trimmed_rhs.starts_with('{');
    let rhs_is_list = trimmed_rhs.starts_with('[');

    let inner_is_list = trimmed_inner.starts_with("list") || trimmed_inner.starts_with("List");
    let inner_is_dict = trimmed_inner.starts_with("dict") || trimmed_inner.starts_with("Dict");
    let inner_is_set = trimmed_inner.starts_with("set")
        || trimmed_inner.starts_with("Set")
        || trimmed_inner.starts_with("frozenset")
        || trimmed_inner.starts_with("FrozenSet");

    // Dict literal assigned to list/set/tuple type
    if rhs_is_dict && (inner_is_list || inner_is_set) {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a dict literal",
            ),
            name_span,
            path,
        ));
        return;
    }

    // List literal assigned to dict type
    if rhs_is_list && inner_is_dict {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a list literal",
            ),
            name_span,
            path,
        ));
    }
}

/// Check module-level attribute assignments to ClassVar-annotated class attributes.
///
/// e.g. `enterprise_d.stats = {}` where `stats: ClassVar[dict[str, int]]` in the class.
fn check_instance_classvar_assignments(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Build a map of class names to their ClassVar attribute names
    let mut classvar_attrs: Vec<(&str, Vec<&str>)> = Vec::new();
    for cls in &module.classes {
        let cv_names: Vec<&str> = cls
            .attributes
            .iter()
            .filter(|attr| {
                span_text(source, attr.annotation_span).is_some_and(|ann| {
                    ann.starts_with("ClassVar[")
                        || ann.starts_with("ClassVar ")
                        || ann == "ClassVar"
                        || ann.starts_with("CV[")
                        || ann == "CV"
                })
            })
            .map(|attr| attr.name.as_str())
            .collect();
        if !cv_names.is_empty() {
            classvar_attrs.push((&cls.name, cv_names));
        }
    }

    if classvar_attrs.is_empty() {
        return;
    }

    // Build a map of variable names to their class types (simple heuristic)
    // Look for module-level assignments like `enterprise_d = Starship(3000)`
    let mut instance_class_map: Vec<(String, String)> = Vec::new();
    for var in &module.module_vars {
        if let Some(rhs) = span_text(source, var.rhs_span) {
            // Check if RHS is a constructor call: ClassName(...)
            if let Some(paren_idx) = rhs.find('(') {
                let class_name = rhs[..paren_idx].trim();
                if !class_name.is_empty()
                    && class_name
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_uppercase())
                    && class_name
                        .chars()
                        .all(|ch| ch.is_alphanumeric() || ch == '_')
                {
                    instance_class_map.push((var.name.clone(), class_name.to_owned()));
                }
            }
        }
    }

    // Check each module-level attribute assignment
    for assignment in &module.module_attr_assignments {
        // Find if the object is an instance of a class with ClassVar attrs
        let Some(class_name) = instance_class_map
            .iter()
            .find(|(var_name, _)| var_name == &assignment.object_name)
            .map(|(_, cls)| cls.as_str())
        else {
            // Could also be a direct class assignment (Starship.stats = {}) which is OK
            continue;
        };

        // Check if the attribute is a ClassVar
        let is_classvar_attr = classvar_attrs.iter().any(|(cls, attrs)| {
            *cls == class_name && attrs.contains(&assignment.attr_name.as_str())
        });

        if is_classvar_attr {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot assign to `ClassVar` attribute `{}` through an instance of `{}`",
                    assignment.attr_name, class_name
                ),
                span: assignment.target_span,
                path: path.to_owned(),
                help: Some(
                    "Assign to the class directly instead: `ClassName.attr = value`".to_owned(),
                ),
                note: Some(
                    "PEP 526: ClassVar attributes can only be assigned on the class itself, \
                     not through instances"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check module-level annotated assignments for protocol ClassVar conformance.
///
/// When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
/// implementation class must have those attributes defined at the **class level**
/// (not merely as `self.x = ...` in `__init__`).
///
/// e.g. `a: ProtoA = ProtoAImpl()` where `ProtoA` requires `y: ClassVar[str]`
/// but `ProtoAImpl` only sets `self.y = ""` in `__init__` (instance variable).
fn check_protocol_classvar_conformance(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Step 1: Build a map of protocol classes -> their ClassVar attribute names.
    let mut protocol_classvar_attrs: Vec<(&str, Vec<&str>)> = Vec::new();
    for cls in &module.classes {
        if !cls.bases.iter().any(|b| b == "Protocol") {
            continue;
        }
        let cv_names: Vec<&str> = cls
            .attributes
            .iter()
            .filter(|attr| {
                span_text(source, attr.annotation_span).is_some_and(|ann| {
                    ann.starts_with("ClassVar[")
                        || ann.starts_with("ClassVar ")
                        || ann == "ClassVar"
                        || ann.starts_with("CV[")
                        || ann == "CV"
                })
            })
            .map(|attr| attr.name.as_str())
            .collect();
        if !cv_names.is_empty() {
            protocol_classvar_attrs.push((&cls.name, cv_names));
        }
    }

    if protocol_classvar_attrs.is_empty() {
        return;
    }

    // Step 2: Build a map of non-protocol class names -> their class-level attribute names.
    let class_level_attrs: Vec<(&str, Vec<&str>)> = module
        .classes
        .iter()
        .filter(|cls| !cls.bases.iter().any(|b| b == "Protocol"))
        .map(|cls| {
            let attr_names: Vec<&str> = cls.attributes.iter().map(|a| a.name.as_str()).collect();
            (cls.name.as_str(), attr_names)
        })
        .collect();

    // Step 3: Check module-level annotated assignments like `a: ProtoName = ClassName(...)`.
    for var in &module.module_vars {
        // Get the annotation text (e.g. "ProtoA").
        let Some(ann_text) = span_text(source, var.annotation_span) else {
            continue;
        };
        let ann_trimmed = ann_text.trim();

        // Check if the annotation names a protocol with ClassVar attrs.
        let Some((_proto_name, required_cv_attrs)) = protocol_classvar_attrs
            .iter()
            .find(|(name, _)| *name == ann_trimmed)
        else {
            continue;
        };

        // Get the RHS text and check if it's a constructor call.
        let Some(rhs_text) = span_text(source, var.rhs_span) else {
            continue;
        };
        let rhs_trimmed = rhs_text.trim();

        // Extract class name from constructor call: ClassName(...)
        let Some(paren_idx) = rhs_trimmed.find('(') else {
            continue;
        };
        let impl_class_name = rhs_trimmed[..paren_idx].trim();
        if impl_class_name.is_empty()
            || !impl_class_name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
            || !impl_class_name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_')
        {
            continue;
        }

        // Find the implementation class's class-level attributes.
        let Some((_cls_name, cls_attrs)) = class_level_attrs
            .iter()
            .find(|(name, _)| *name == impl_class_name)
        else {
            continue;
        };

        // Check each required ClassVar attribute.
        for cv_attr in required_cv_attrs {
            if !cls_attrs.contains(cv_attr) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class `{impl_class_name}` is not compatible with protocol \
                         `{ann_trimmed}`: attribute `{cv_attr}` is required to be a \
                         class variable (`ClassVar`) but is not defined at class level",
                    ),
                    span: var.name_span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Define `{cv_attr}` as a class-level attribute in \
                         `{impl_class_name}` instead of assigning via `self.{cv_attr}` \
                         in `__init__`",
                    )),
                    note: Some(
                        "Protocol `ClassVar` attributes must be class-level variables \
                         in the implementation, not instance variables"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}
