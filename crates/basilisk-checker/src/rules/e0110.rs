//! BSK-E0110: Protocol variance violation.
//!
//! Detects when a Protocol class declares `TypeVar`s with incorrect variance
//! based on how they are used in method signatures:
//!
//! - A `TypeVar` used only in output positions (return types) should be covariant.
//! - A `TypeVar` used only in input positions (parameters) should be contravariant.
//! - A covariant `TypeVar` used in input position is a violation.
//! - A contravariant `TypeVar` used in output position is a violation.
//!
//! `__init__` and `__new__` methods are exempt from variance inference.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use super::guards::is_protocol_class;
use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0110",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0110",
};

/// Methods exempt from variance inference per the typing spec.
const EXEMPT_METHODS: &[&str] = &["__init__", "__new__"];

/// Known covariant containers -- a `TypeVar` inside one of these in a return
/// annotation is still purely in output position.
const COVARIANT_CONTAINERS: &[&str] = &[
    "type",
    "Type",
    "tuple",
    "Tuple",
    "FrozenSet",
    "frozenset",
    "Sequence",
    "Iterator",
    "Iterable",
    "Mapping",
    "AbstractSet",
    "Collection",
];

/// The variance of a `TypeVar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variance {
    Invariant,
    Covariant,
    Contravariant,
}

/// BSK-E0110: Protocol variance violation.
pub(crate) struct ProtocolVarianceViolation;

/// Check whether `text` contains `name` as a whole word (not as a substring
/// of a longer identifier).
fn contains_typevar(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let name_len = name_bytes.len();

    if name_len > text_bytes.len() {
        return false;
    }

    for start in 0..=(text_bytes.len() - name_len) {
        let Some(slice) = text_bytes.get(start..start + name_len) else {
            continue;
        };
        if slice != name_bytes {
            continue;
        }
        if start > 0 {
            if let Some(&left) = text_bytes.get(start - 1) {
                if left.is_ascii_alphanumeric() || left == b'_' {
                    continue;
                }
            }
        }
        let end = start + name_len;
        if end < text_bytes.len() {
            if let Some(&right) = text_bytes.get(end) {
                if right.is_ascii_alphanumeric() || right == b'_' {
                    continue;
                }
            }
        }
        return true;
    }
    false
}

/// Extract the text for a span from source, returning `None` if out of range.
fn span_text(source: &str, span: basilisk_resolver::Span) -> Option<&str> {
    slice_span(source, span)
}

/// Check whether a `TypeVar` appears inside a generic container that is NOT
/// covariant (i.e. invariant or contravariant), meaning the `TypeVar` is
/// effectively used in both input and output positions.
fn typevar_in_invariant_container(text: &str, tv_name: &str) -> bool {
    if !contains_typevar(text, tv_name) {
        return false;
    }
    let trimmed = text.trim();
    if trimmed == tv_name {
        return false;
    }
    // No generic brackets means a simple union like `T1 | T2`.
    if !trimmed.contains('[') {
        return false;
    }
    // Check if the container is a known covariant type.
    for container in COVARIANT_CONTAINERS {
        let prefix = format!("{container}[");
        if trimmed.starts_with(&prefix) {
            return false;
        }
    }
    true
}

/// Collect `TypeVar` names appearing in annotation text, distinguishing
/// direct usage from usage nested in invariant containers.
fn collect_typevars_in_annotation<'a>(
    text: &str,
    param_names: &[&'a str],
    direct: &mut HashSet<&'a str>,
    nested_invariant: &mut HashSet<&'a str>,
) {
    for &tv_name in param_names {
        if !contains_typevar(text, tv_name) {
            continue;
        }
        if typevar_in_invariant_container(text, tv_name) {
            let _ = nested_invariant.insert(tv_name);
        } else {
            let _ = direct.insert(tv_name);
        }
    }
}

/// Scan all input-position annotations of a method for `TypeVar` occurrences.
///
/// `TypeVars` inside invariant containers are variance-neutral and do NOT count
/// as input usage — only direct (bare or covariant-container) occurrences do.
fn scan_input_positions<'a>(
    method: &FunctionInfo,
    source: &str,
    param_names: &[&'a str],
    input_usage: &mut HashSet<&'a str>,
) {
    // Regular parameters (skip self/cls).
    for param in &method.parameters {
        if param.name == "self" || param.name == "cls" {
            continue;
        }
        if let Some(ann_span) = param.annotation_span {
            if let Some(ann_text) = span_text(source, ann_span) {
                let mut direct = HashSet::new();
                let mut nested = HashSet::new();
                collect_typevars_in_annotation(ann_text, param_names, &mut direct, &mut nested);
                input_usage.extend(direct);
            }
        }
    }

    // Vararg and kwarg parameters.
    for vk_param in [&method.vararg, &method.kwarg].into_iter().flatten() {
        if let Some(ann_span) = vk_param.annotation_span {
            if let Some(ann_text) = span_text(source, ann_span) {
                let mut direct = HashSet::new();
                let mut nested = HashSet::new();
                collect_typevars_in_annotation(ann_text, param_names, &mut direct, &mut nested);
                input_usage.extend(direct);
            }
        }
    }
}

/// Scan the return annotation of a method for `TypeVar` occurrences.
///
/// `TypeVars` inside invariant containers are variance-neutral and do NOT count
/// as either input or output usage — only direct (bare or covariant-container)
/// occurrences count as output usage.
fn scan_output_position<'a>(
    method: &FunctionInfo,
    source: &str,
    param_names: &[&'a str],
    output_usage: &mut HashSet<&'a str>,
) {
    let Some(ret_span) = method.return_annotation_span else {
        return;
    };
    let Some(ret_text) = span_text(source, ret_span) else {
        return;
    };

    let mut direct = HashSet::new();
    let mut nested_invariant = HashSet::new();
    collect_typevars_in_annotation(ret_text, param_names, &mut direct, &mut nested_invariant);

    // Direct usage in return is output.
    output_usage.extend(direct);

    // TypeVars only inside invariant containers are variance-neutral — skip.
}

/// Context for emitting variance diagnostics on a single protocol class.
struct VarianceContext<'a> {
    cls: &'a ClassInfo,
    methods: &'a [&'a FunctionInfo],
    path: &'a str,
}

impl VarianceContext<'_> {
    /// Emit variance violation diagnostics for a single `TypeVar`.
    fn emit_diagnostics(
        &self,
        tv_name: &str,
        declared_variance: Variance,
        in_input: bool,
        in_output: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match declared_variance {
            Variance::Invariant => {
                self.emit_invariant_violations(tv_name, in_input, in_output, diagnostics);
            }
            Variance::Covariant if in_input => {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Covariant TypeVar `{tv_name}` is used in input position \
                         in protocol `{}`",
                        self.cls.name
                    ),
                    self.cls.def_span,
                    self.path,
                    Some(
                        "Covariant TypeVars should only appear in output \
                         (return type) positions"
                            .to_owned(),
                    ),
                    None,
                ));
            }
            Variance::Contravariant if in_output => {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Contravariant TypeVar `{tv_name}` is used in output \
                         position in protocol `{}`",
                        self.cls.name
                    ),
                    self.cls.def_span,
                    self.path,
                    Some(
                        "Contravariant TypeVars should only appear in input \
                         (parameter) positions"
                            .to_owned(),
                    ),
                    None,
                ));
            }
            _ => {}
        }
    }

    /// Emit diagnostics for an invariant `TypeVar` that should be co-/contravariant.
    fn emit_invariant_violations(
        &self,
        tv_name: &str,
        in_input: bool,
        in_output: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if in_output && !in_input {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "TypeVar `{tv_name}` in protocol `{}` is only used in \
                     output positions and should be covariant",
                    self.cls.name
                ),
                self.cls.def_span,
                self.path,
                Some(format!("Declare `{tv_name}` with `covariant=True`")),
                None,
            ));
        }
        if in_input && !in_output {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "TypeVar `{tv_name}` in protocol `{}` is only used in \
                     input positions and should be contravariant",
                    self.cls.name
                ),
                self.cls.def_span,
                self.path,
                Some(format!("Declare `{tv_name}` with `contravariant=True`")),
                None,
            ));
        }
        // All methods are exempt (e.g. only __init__): TypeVar is effectively covariant.
        if !in_input && !in_output {
            let has_non_exempt = self
                .methods
                .iter()
                .any(|m| !EXEMPT_METHODS.contains(&m.name.as_str()));
            if !has_non_exempt && !self.methods.is_empty() {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "TypeVar `{tv_name}` in protocol `{}` is only used in \
                         output positions and should be covariant",
                        self.cls.name
                    ),
                    self.cls.def_span,
                    self.path,
                    Some(format!("Declare `{tv_name}` with `covariant=True`")),
                    None,
                ));
            }
        }
    }
}

impl Rule for ProtocolVarianceViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;

        // Build TypeVar name -> variance map (skip ParamSpec and TypeVarTuple).
        let tv_variance: HashMap<&str, Variance> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_paramspec && !tv.is_typevartuple)
            .map(|tv| {
                let variance = if tv.is_covariant {
                    Variance::Covariant
                } else if tv.is_contravariant {
                    Variance::Contravariant
                } else {
                    Variance::Invariant
                };
                (tv.name.as_str(), variance)
            })
            .collect();

        for cls in &module.classes {
            if !is_protocol_class(cls) || cls.generic_params.is_empty() {
                continue;
            }

            // Only TypeVar params (skip ParamSpec/TypeVarTuple).
            let param_names: Vec<&str> = cls
                .generic_params
                .iter()
                .filter(|gp| tv_variance.contains_key(gp.name.as_str()))
                .map(|gp| gp.name.as_str())
                .collect();

            if param_names.is_empty() {
                continue;
            }

            let mut input_usage: HashSet<&str> = HashSet::new();
            let mut output_usage: HashSet<&str> = HashSet::new();

            let methods: Vec<&FunctionInfo> = module
                .functions
                .iter()
                .filter(|f| f.class_name.as_deref() == Some(cls.name.as_str()))
                .collect();

            for method in &methods {
                if EXEMPT_METHODS.contains(&method.name.as_str()) {
                    continue;
                }
                scan_input_positions(method, source, &param_names, &mut input_usage);
                scan_output_position(method, source, &param_names, &mut output_usage);
            }

            let ctx = VarianceContext {
                cls,
                methods: &methods,
                path: &module.path,
            };

            for &tv_name in &param_names {
                let Some(&declared_variance) = tv_variance.get(tv_name) else {
                    continue;
                };
                ctx.emit_diagnostics(
                    tv_name,
                    declared_variance,
                    input_usage.contains(tv_name),
                    output_usage.contains(tv_name),
                    diagnostics,
                );
            }
        }
    }
}
