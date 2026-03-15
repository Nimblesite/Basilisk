//! BSK-E0149: PEP 695 generic type parameter scoping violations.
//!
//! Detects violations of PEP 695 type parameter scoping rules:
//!
//! 1. A type parameter's bound references another type parameter in the same
//!    parameter list that has not yet been defined (forward reference in bounds).
//!    Per PEP 695: "A compiler error or runtime exception is generated if the
//!    definition of an earlier type parameter references a later type parameter."
//!
//! 2. A PEP 695 type parameter is used at module level or in a decorator
//!    applied to a generic construct, outside the scope where the type parameter
//!    is defined.
//!
//! 3. A method inside a generic class defines its own type parameter with the
//!    same name as the enclosing class's type parameter, creating a shadowing
//!    conflict.
//!
//! ```python
//! class ClassA[S, T: Sequence[S]]:  # E — T's bound references S (earlier param)
//!     ...
//!
//! class ClassB[S: Sequence[T], T]:  # E — S's bound references T (later param)
//!     ...
//!
//! print(T)  # E — T is not defined at module scope
//!
//! @decorator(Foo[T])  # E — T not in scope in decorator
//! class ClassD[T]: ...
//!
//! class ClassE[T]:
//!     def method1[T](self): ...  # E — method re-defines class type param T
//! ```
//!
//! Reference: <https://peps.python.org/pep-0695/#type-parameter-scopes>

mod helpers;
mod violations;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

use helpers::leading_indent;
use violations::{
    check_decorator_uses_class_type_param, check_method_redefines_class_type_param,
    check_module_level_type_param_use, check_pep695_bound_cross_references,
    check_type_stmt_circular, check_type_stmt_in_function, check_type_stmt_uses_old_typevar,
    collect_pep695_type_params,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0149",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0149",
};

/// Emits BSK-E0149 for PEP 695 generic type parameter scoping violations.
pub(crate) struct Pep695TypeParamScopingViolation;

impl Rule for Pep695TypeParamScopingViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let lines: Vec<&str> = source.lines().collect();

        // Collect all PEP 695 type params defined anywhere in the file.
        let all_pep695_params = collect_pep695_type_params(source);

        // Collect old-style TypeVar names (from TypeVar() calls) for
        // old/new mixing detection.
        let old_typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        for (line_idx, &line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // --- Violation 1: cross-references in type param bounds ---
            if trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
            {
                check_pep695_bound_cross_references(line, line_number, source, path, diagnostics);
            }

            // --- Violation 3: method re-defines class type param ---
            if trimmed.starts_with("class ") {
                check_method_redefines_class_type_param(
                    &lines,
                    line_number,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 2a: module-level use of PEP 695 type param ---
            if leading_indent(line) == 0 && !all_pep695_params.is_empty() {
                check_module_level_type_param_use(
                    line,
                    line_number,
                    &all_pep695_params,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 2b: decorator uses the decorated class's type param ---
            if trimmed.starts_with('@') && leading_indent(line) == 0 {
                check_decorator_uses_class_type_param(
                    &lines,
                    line_number,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 4: `type` statement uses old-style TypeVar ---
            if trimmed.starts_with("type ") && !old_typevar_names.is_empty() {
                check_type_stmt_uses_old_typevar(
                    line,
                    line_number,
                    &old_typevar_names,
                    source,
                    path,
                    diagnostics,
                );
            }

            // --- Violation 5: `type` statement inside function body ---
            if trimmed.starts_with("type ") {
                check_type_stmt_in_function(line, line_number, source, path, diagnostics);
            }

            // --- Violation 6: circular type alias definition ---
            if trimmed.starts_with("type ") {
                check_type_stmt_circular(line, line_number, source, path, diagnostics);
            }
        }

        // --- Violation 7: misuse of PEP 695 type aliases ---
        check_type_alias_misuse(module, diagnostics);
    }
}

/// Check for misuse of PEP 695 type aliases: calling, subclassing,
/// `isinstance()`, and attribute access (except `__value__` / `__type_params__`).
fn check_type_alias_misuse(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Collect PEP 695 type alias names
    let alias_names: std::collections::HashSet<&str> = module
        .type_statements
        .iter()
        .map(|ts| ts.name.as_str())
        .collect();

    if alias_names.is_empty() {
        return;
    }

    // Check calls: `Alias()` is not allowed
    for call in &module.calls {
        if alias_names.contains(call.callee.as_str()) {
            // `isinstance(x, Alias)` — special case
            if call.callee == "isinstance" || call.callee == "issubclass" {
                continue; // Handled below via argument checking
            }
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot call type alias `{}`: type aliases are not callable",
                    call.callee
                ),
                span: call.span,
                path: path.to_owned(),
                help: Some("Type aliases created with `type` cannot be instantiated".to_owned()),
                note: None,
            });
        }

        // Check isinstance/issubclass with alias as second arg
        if (call.callee == "isinstance" || call.callee == "issubclass") && call.args.len() >= 2 {
            if let Some((_, arg_span)) = call.args.get(1) {
                if let Some(arg_text) = crate::span_util::slice_span(source, *arg_span) {
                    let arg_trimmed = arg_text.trim();
                    if alias_names.contains(arg_trimmed) {
                        diagnostics.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Cannot use type alias `{arg_trimmed}` in `{}`",
                                call.callee
                            ),
                            span: *arg_span,
                            path: path.to_owned(),
                            help: Some(format!(
                                "Type aliases created with `type` cannot be used with `{}`",
                                call.callee
                            )),
                            note: None,
                        });
                    }
                }
            }
        }
    }

    // Check class bases: `class Foo(Alias)` is not allowed
    for class in &module.classes {
        for base in &class.bases {
            let base_name = base.split('[').next().unwrap_or(base).trim();
            if alias_names.contains(base_name) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!("Cannot use type alias `{base_name}` as a base class"),
                    span: class.name_span,
                    path: path.to_owned(),
                    help: Some("Type aliases created with `type` cannot be subclassed".to_owned()),
                    note: None,
                });
            }
        }
    }

    check_alias_attribute_access(&alias_names, source, path, diagnostics);
}

/// Check attribute access on type aliases (source-level scan).
///
/// Only `__value__` and `__type_params__` are allowed.
fn check_alias_attribute_access(
    alias_names: &std::collections::HashSet<&str>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = source.lines().collect();
    for (line_idx, &line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();
        let before_comment = trimmed.split_once('#').map_or(trimmed, |(code, _)| code);

        for alias_name in alias_names {
            let pattern = format!("{alias_name}.");
            if let Some(pos) = before_comment.find(&pattern) {
                let after_dot = &before_comment[pos + pattern.len()..];
                let attr: String = after_dot
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                if attr == "__value__" || attr == "__type_params__" {
                    continue;
                }

                if trimmed.starts_with("type ") {
                    continue;
                }

                if !attr.is_empty() {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Cannot access attribute `{attr}` on type alias `{alias_name}`"
                        ),
                        span: helpers::span_for_line(source, line_number),
                        path: path.to_owned(),
                        help: Some(
                            "Type aliases only support `__value__` and `__type_params__` \
                             attributes"
                                .to_string(),
                        ),
                        note: None,
                    });
                }
            }
        }
    }
}
