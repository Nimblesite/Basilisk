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

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

use helpers::leading_indent;
use violations::{
    check_decorator_uses_class_type_param, check_method_redefines_class_type_param,
    check_module_level_type_param_use, check_pep695_bound_cross_references,
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
        }
    }
}
