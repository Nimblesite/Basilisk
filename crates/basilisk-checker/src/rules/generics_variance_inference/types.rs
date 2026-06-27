//! Implements [generics_variance_inference] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Data types for generics_variance_inference.

use std::collections::{HashMap, HashSet};

/// Represents a scope (class or function) with its indentation level and bound `TypeVar`s.
pub(super) struct ScopeInfo {
    /// Indentation column of the `class`/`def` keyword.
    pub(super) indent: usize,
    /// `TypeVar` names bound by this scope (from `Generic[...]` params or function annotations).
    pub(super) bound_typevars: HashSet<String>,
    /// Whether this is a class scope (vs function scope).
    pub(super) is_class: bool,
}

/// A generic class definition discovered from source text.
pub(super) struct GenericClassDef {
    /// The class name.
    pub(super) name: String,
    /// `TypeVar` names in `Generic[T, S, ...]` order.
    pub(super) typevar_params: Vec<String>,
    /// Methods: name -> list of `(param_name, annotation_text)` pairs (excluding `self`).
    pub(super) methods: HashMap<String, Vec<(String, String)>>,
}

/// A module-level variable annotated with a concrete generic type.
pub(super) struct GenericInstance {
    /// The variable name (e.g. `a`).
    pub(super) var_name: String,
    /// The class name (e.g. `MyClass`).
    pub(super) class_name: String,
    /// The concrete type args in order (e.g. `["int"]`).
    pub(super) type_args: Vec<String>,
}
