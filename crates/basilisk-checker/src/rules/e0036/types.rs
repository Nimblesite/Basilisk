//! Types for BSK-E0036.

/// Classification of a type parameter for error messaging.
#[derive(Debug, Clone, Copy)]
pub(super) enum TypeParamKind {
    TypeVar,
    ParamSpec,
    TypeVarTuple,
}
