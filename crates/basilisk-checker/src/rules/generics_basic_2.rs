//! Implements [`generics_basic_2`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `generics_basic_2`: Non-TypeVar argument in `Generic[...]` or `Protocol[...]`.
//!
//! PEP 484 requires that all arguments to `Generic[...]` and `Protocol[...]`
//! be type variable names (`TypeVar`, `TypeVarTuple`, or `ParamSpec`).
//! Passing a concrete type (e.g. `Generic[int]`) is a type error.
//!
//! ```python
//! class Bad1(Generic[int]): ...      # E — `int` is not a TypeVar
//! class Bad2(Protocol[int]): ...     # E — `int` is not a TypeVar
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_basic_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_basic_2",
};

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "All arguments to `Generic[...]` must be TypeVar, TypeVarTuple, \
             or ParamSpec instances"
                .to_owned(),
        ),
        Some("PEP 484: `Generic[int]` is invalid; use a TypeVar instead".to_owned()),
    )
}

/// Emits `generics_basic_2` when a non-TypeVar appears in `Generic[...]` or `Protocol[...]`.
pub(crate) struct NonTypeVarInGeneric;

impl Rule for NonTypeVarInGeneric {
    // ##########################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // # `ClassInfo::base_expression_names` is a `Vec<String>` of RENDERED simple
    // # names harvested from base-class expressions. This code matched those
    // # strings against a set of TypeVar names collected the same way, so:
    // #
    // #   T = TypeVar("T")
    // #   Alias = T
    // #   class Foo(Generic[Alias]): ...      # TypeVar NOT recognised
    // #
    // #   class T: ...                        # unrelated class
    // #   class Foo(Base[T]): ...             # treated as a TypeVar use
    // #
    // # Whether a base-expression name denotes a TypeVar is a question about the
    // # binding it resolves to, not about the characters written.
    // #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
    // ##########################################################################
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        panic!(
        "basilisk-checker: `generics_basic_2::check` was DELETED because it matched TypeVar identity by \
         RENDERED NAME against `base_expression_names`, so an aliased TypeVar was \
         invisible and any unrelated symbol spelled like one matched. It panics \
         because the real implementation — base expressions resolved through the \
         binding table — DOES NOT EXIST YET. Do not restore the name matching and \
         do not substitute a default answer."
    )
    }
}
