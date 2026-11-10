//! Implements [`protocols_variance`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_variance`: Protocol variance violation.
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

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

const CODE: ErrorCode = ErrorCode {
    code: "protocols_variance",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_variance",
};

/// Methods exempt from variance inference per the typing spec.
const EXEMPT_METHODS: &[&str] = &["__init__", "__new__"];

// DELETED — `COVARIANT_CONTAINERS`, a table of builtin SPELLINGS used to build
// `format!("{container}[")` prefixes and match them against annotation text.
// Its only reader panics; see the banner below. DO NOT RECREATE IT.

/// The variance of a `TypeVar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variance {
    Invariant,
    Covariant,
    Contravariant,
}

/// `protocols_variance`: Protocol variance violation.
pub(crate) struct ProtocolVarianceViolation;

// ##########################################################################
// # DELETED BODY — `contains_typevar`. DO NOT RESTORE IT AND DO NOT RETURN #
// # EITHER ANSWER UNCONDITIONALLY.                                         #
// #                                                                        #
// # A SECOND HAND-WRITTEN REGEX ENGINE over Python source bytes — the same #
// # construct, byte for byte in shape, as the `is_word_boundary_match`     #
// # already deleted from `generics_syntax_compatibility`. It slid a window #
// # across `text.as_bytes()`, compared it to the TypeVar's name, and faked #
// # a word boundary by testing the neighbouring bytes:                     #
// #                                                                        #
// #   if left.is_ascii_alphanumeric() || left == b'_' { continue; }        #
// #                                                                        #
// # CLAUDE.md: "Never parse with strings or regex". Concretely this        #
// # answered `true` for a TypeVar named `T` appearing inside a STRING      #
// # LITERAL (`Literal["T"]`), inside a forward reference, or inside a      #
// # comment carried in the slice; and the ASCII-only boundary splits the   #
// # non-ASCII identifiers Python permits, so a TypeVar `T` was found       #
// # inside the perfectly distinct name `Tπ`.                              #
// #                                                                        #
// # Whether a TypeVar occurs in an annotation is a question about the      #
// # annotation's EXPRESSION TREE and the binding each `Expr::Name` in it   #
// # resolves to — which also makes `Alias = T; def f(x: Alias)` visible,   #
// # and this never could.                                                  #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn contains_typevar(_text: &str, _name: &str) -> bool {
    panic!(
        "basilisk-checker: `contains_typevar` was DELETED because it was a \
         hand-written regex over Python SOURCE BYTES — a sliding window plus an \
         ASCII-only word-boundary test — so a TypeVar's name matched inside string \
         literals and forward references, and an aliased TypeVar matched nowhere. It \
         panics because the real implementation — walking the annotation's expression \
         tree and resolving each name through the binding table — DOES NOT EXIST YET. \
         Do not restore the byte scan and do not pick a constant answer in its place."
    )
}

/// Extract the text for a span from source, returning `None` if out of range.
fn span_text(source: &str, span: basilisk_resolver::Span) -> Option<&str> {
    slice_span(source, span)
}

// ##########################################################################
// # DELETED BODY — `typevar_in_invariant_container`. DO NOT RESTORE IT.
// #
// # Variance was decided by STRING SURGERY on annotation source text:
// #
// #   if !trimmed.contains('[') { return false; }
// #   for container in COVARIANT_CONTAINERS {          // "type","tuple","frozenset"
// #       let prefix = format!("{container}[");
// #       if trimmed.starts_with(&prefix) { return false; }
// #   }
// #
// # It tested whether rendered text BEGINS WITH a builtin's spelling followed
// # by a bracket. `tuple [T]` (space), `Tuple[T]`, an aliased import, or a
// # user class named `tuple` all got the wrong answer, and a generic written
// # without brackets was declared non-generic. Whether a container is
// # covariant is a property of the RESOLVED class, not of its rendering.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn typevar_in_invariant_container(_text: &str, _tv_name: &str) -> bool {
    panic!(
        "basilisk-checker: `typevar_in_invariant_container` was DELETED because it \
         decided variance by testing whether annotation TEXT starts with a builtin \
         container's spelling followed by `[`. It panics because the real \
         implementation — the resolved class's declared variance, read from the binding \
         table — DOES NOT EXIST YET. Do not restore the prefix test and do not \
         substitute a default answer."
    )
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
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
            if !cls.is_protocol || cls.generic_params.is_empty() {
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
