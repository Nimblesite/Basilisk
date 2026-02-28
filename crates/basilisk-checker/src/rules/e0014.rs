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

use basilisk_resolver::{ResolvedModule, RhsKind, Span, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0014",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0014",
};

/// Emits BSK-E0014 for annotated module variables whose annotation and literal
/// RHS are obviously incompatible.
pub(crate) struct AssignmentTypeMismatch;

impl Rule for AssignmentTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| {
                let annotation = extract_annotation(&module.source, var.name_span)?;
                let mismatch = annotation_rhs_mismatch(annotation, &var.rhs_kind)?;
                Some((var, annotation.to_owned(), mismatch))
            })
            .for_each(|(var, annotation, mismatch)| {
                diagnostics.push(make_diagnostic(var, &annotation, mismatch, &module.path));
            });
    }
}

/// Describes why an annotation/RHS pair is incompatible.
struct Mismatch {
    rhs_description: &'static str,
}

/// Returns `Some(Mismatch)` when the annotation text and RHS kind are
/// clearly incompatible; `None` when the pairing is acceptable or unknown.
fn annotation_rhs_mismatch<'a>(annotation: &str, rhs: &RhsKind) -> Option<Mismatch> {
    // Normalise: strip generic parameters and whitespace, lower-case.
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        // int annotation with a string, bytes, or bool-string value
        ("int", RhsKind::StrLiteral) => Some(Mismatch { rhs_description: "a `str` literal" }),
        ("int", RhsKind::BytesLiteral) => Some(Mismatch { rhs_description: "a `bytes` literal" }),
        ("int", RhsKind::FloatLiteral) => {
            Some(Mismatch { rhs_description: "a `float` literal" })
        }

        // str annotation with a numeric value
        ("str", RhsKind::IntLiteral) => Some(Mismatch { rhs_description: "an `int` literal" }),
        ("str", RhsKind::FloatLiteral) => {
            Some(Mismatch { rhs_description: "a `float` literal" }
            )
        }
        ("str", RhsKind::BytesLiteral) => {
            Some(Mismatch { rhs_description: "a `bytes` literal" })
        }

        // bool annotation with a string value
        ("bool", RhsKind::StrLiteral) => Some(Mismatch { rhs_description: "a `str` literal" }),
        ("bool", RhsKind::FloatLiteral) => {
            Some(Mismatch { rhs_description: "a `float` literal" })
        }

        // float annotation with a string value
        ("float", RhsKind::StrLiteral) => Some(Mismatch { rhs_description: "a `str` literal" }),
        ("float", RhsKind::BytesLiteral) => {
            Some(Mismatch { rhs_description: "a `bytes` literal" })
        }

        // bytes annotation with a string value
        ("bytes", RhsKind::StrLiteral) => Some(Mismatch { rhs_description: "a `str` literal" }),
        ("bytes", RhsKind::IntLiteral) => Some(Mismatch { rhs_description: "an `int` literal" }),

        _ => None,
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
fn extract_annotation<'src>(source: &'src str, name_span: Span) -> Option<&'src str> {
    // Find the byte offset of the start of the line containing the name.
    let start = name_span.start as usize;
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line[name_offset..].find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
    let eq_pos = line[after_colon..].find('=').map(|p| after_colon + p);

    let annotation_end = eq_pos.unwrap_or(line.len());
    let annotation = line.get(after_colon..annotation_end)?.trim();

    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

fn make_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    mismatch: Mismatch,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Type mismatch: `{}` is annotated `{}` but assigned {}",
            var.name, annotation, mismatch.rhs_description
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Either change the annotation to match the value, or change the value to `{}`",
            annotation
        )),
        note: Some(
            "Basilisk requires the literal kind to be compatible with the declared type"
                .to_owned(),
        ),
    }
}
