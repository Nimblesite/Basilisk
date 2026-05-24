//! BSK-E0149: PEP 695 generic type parameter scoping violations.
//!
//! Detects violations of PEP 695 type-parameter scoping rules, driven entirely
//! by `ruff_python_ast` nodes (via [`basilisk_resolver::Pep695Scoping`]) — never
//! by raw `source.lines()` scanning, so docstring/comment/string content is
//! never mistaken for real `class` / `def` / `type` declarations.
//!
//! 1. A type parameter's bound references another type parameter in the same
//!    list (forward or backward reference).
//! 2. A type parameter is used at module scope (2a) or in a decorator applied to
//!    the generic construct that declares it (2b).
//! 3. A method re-declares an enclosing class's type parameter (shadowing).
//! 4. A `type` statement references an old-style `TypeVar`.
//! 5. A `type` statement appears inside a function body.
//! 6. A `type` alias is circular.
//! 7. A `type` alias is misused (called, subclassed, `isinstance`, attribute).
//! 8. A type argument violates a bounded alias type parameter.
//!
//! ```python
//! class ClassA[S, T: Sequence[S]]: ...  # E — T's bound references S
//! print(T)                              # E — T not defined at module scope
//!
//! @decorator(Foo[T])                    # E — T not in scope in the decorator
//! class ClassD[T]: ...
//!
//! class ClassE[T]:
//!     def method1[T](self): ...         # E — method re-defines class type param
//! ```
//!
//! Reference: <https://peps.python.org/pep-0695/#type-parameter-scopes>

mod alias_misuse;
mod violations;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0149",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0149",
};

/// Emits BSK-E0149 for PEP 695 generic type parameter scoping violations.
pub(crate) struct Pep695TypeParamScopingViolation;

impl Rule for Pep695TypeParamScopingViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let scoping = &module.pep695_scoping;
        let path = &module.path;

        violations::check_bound_cross_references(scoping, path, diagnostics);
        violations::check_module_level_type_param_use(scoping, path, diagnostics);
        violations::check_decorator_uses_class_type_param(scoping, path, diagnostics);
        violations::check_method_redefines_class_type_param(scoping, path, diagnostics);

        let old_typevar_names = basilisk_resolver::collect_names(&module.typevar_calls);
        violations::check_type_alias_uses_old_typevar(
            scoping,
            &old_typevar_names,
            path,
            diagnostics,
        );
        violations::check_type_alias_in_function(scoping, path, diagnostics);
        violations::check_type_alias_circular(scoping, path, diagnostics);

        alias_misuse::check_type_alias_misuse(module, scoping, diagnostics);
        alias_misuse::check_type_alias_bound_violations(module, scoping, diagnostics);
    }
}
