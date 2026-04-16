//! BSK-E0089: Invalid PEP 695 type parameter bound or constraint.
//!
//! PEP 695 introduced a new syntax for declaring type parameters in class and
//! function definitions.  The bound/constraint expression after `:` is restricted
//! to specific forms; invalid forms are caught by this rule.
//!
//! ```python
//! # BAD
//! class Foo[T: [str, int]]:  # E: list literal is not a valid bound
//!     ...
//!
//! class Bar[T: ()]:  # E: constraint tuple must have two or more types
//!     ...
//!
//! class Baz[T: (str,)]:  # E: constraint tuple must have two or more types
//!     ...
//!
//! t1 = (bytes, str)
//! class Qux[T: t1]:  # E: constraint must be a literal tuple expression
//!     ...
//!
//! class Bad[T: (3, bytes)]:  # E: 3 is not a valid type expression
//!     ...
//! ```

use basilisk_resolver::{Pep695BoundViolationKind, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0089",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0089",
};

/// Emits BSK-E0089 when a PEP 695 type parameter has an invalid bound or constraint.
pub(crate) struct Pep695InvalidBound;

impl Rule for Pep695InvalidBound {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.pep695_bound_violations {
            let message = match &violation.kind {
                Pep695BoundViolationKind::ListLiteralBound => format!(
                    "Type parameter `{}` in class `{}` has a list literal as its bound; \
                     use a tuple for constraints or a single type for an upper bound",
                    violation.type_param_name, violation.class_name
                ),
                Pep695BoundViolationKind::EmptyTuple => format!(
                    "Type parameter `{}` in class `{}` has an empty constraint tuple; \
                     constraint tuples must contain two or more types",
                    violation.type_param_name, violation.class_name
                ),
                Pep695BoundViolationKind::SingleElementTuple => format!(
                    "Type parameter `{}` in class `{}` has a single-element constraint tuple; \
                     constraint tuples must contain two or more types",
                    violation.type_param_name, violation.class_name
                ),
                Pep695BoundViolationKind::NonLiteralConstraint => format!(
                    "Type parameter `{}` in class `{}` uses a non-literal constraint; \
                     the constraint must be a literal tuple expression, not a variable",
                    violation.type_param_name, violation.class_name
                ),
                Pep695BoundViolationKind::InvalidConstraintElement => format!(
                    "Type parameter `{}` in class `{}` has an invalid constraint element; \
                     constraint tuple elements must be types, not literal values",
                    violation.type_param_name, violation.class_name
                ),
                Pep695BoundViolationKind::OuterScopeTypeVarInBound => format!(
                    "Type parameter `{}` in class `{}` references a type variable from an \
                     outer scope in its bound; this is not allowed",
                    violation.type_param_name, violation.class_name
                ),
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: violation.span,
                path: module.path.clone(),
                help: None,
                note: None,
                provenance: None,
            });
        }
    }
}
