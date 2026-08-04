//! Implements [TYPEINF-OVERVIEW], [TYPEINF-SUBTYPING], and
//! [TYPEINF-SPECIAL]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-OVERVIEW
//! Type representation for Basilisk's type inference engine.
//!
//! Annotation parsing logic lives in [`super::types_parsing`].

use std::fmt;

/// Represents an inferred type from Basilisk's type inference engine.
#[derive(Debug, Clone, PartialEq)]
pub enum InferredType {
    /// Integer type (`int`)
    Int,
    /// String type (`str`)
    Str,
    /// Float type (`float`)
    Float,
    /// Boolean type (`bool`)
    Bool,
    /// Bytes type (`bytes`)
    Bytes,
    /// None type (`None`)
    None_,
    /// Literal value type (`Literal[value]`)
    Literal(LiteralValue),
    /// List type (`list[T]`)
    List(Box<InferredType>),
    /// Dictionary type (`dict[K, V]`)
    Dict(Box<InferredType>, Box<InferredType>),
    /// Set type (`set[T]`)
    Set(Box<InferredType>),
    /// Tuple type (`tuple[T1, T2, ...]`)
    Tuple(Vec<InferredType>),
    /// Union type (`T1 | T2`)
    Union(Vec<InferredType>),
    /// Optional type (`Optional[T]` or `T | None`)
    Optional(Box<InferredType>),
    /// Callable type (`Callable[[params...], return]` or `Callable[..., return]`)
    Callable(CallableInfo),
    /// Generator type (`Generator[Yield, Send, Return]`).
    Generator(Box<InferredType>, Box<InferredType>, Box<InferredType>),
    /// Any type (`Any`) - explicit escape hatch.
    /// Implements [TYPEINF-SPECIAL-ANY] — the explicit escape hatch variant; never
    /// inferred as a fallback (unannotated params produce E0001, not `Any`).
    Any,
    /// Never type (`Never`) - bottom type, no values.
    /// Implements [TYPEINF-SPECIAL-NEVER] — bottom type used for always-raises
    /// return inference and pattern-match exhaustiveness.
    Never,
    /// Unknown type - used when type cannot be determined
    Unknown,
    /// `LiteralString` — any string literal or value known to be a literal string.
    /// Implements [TYPEINF-SPECIAL-LITERALSTRING] — supertype of all `Literal[str]`.
    LiteralString,
    /// Named type (`ClassName`) - fallback for named types not yet resolved
    Named(String),
    /// `TypeForm[T]` — represents a type form object for type `T` (PEP 747).
    /// The inner type is what the type form represents (e.g. `TypeForm[int]`
    /// means a type form that represents `int`).
    TypeForm(Box<InferredType>),
    /// `TypeGuard[T]` (PEP 647) or `TypeIs[T]` (PEP 742) — a user-defined
    /// narrowing function's return form. `type_is` distinguishes the PEP 742
    /// bidirectional form (narrows both branches, requires the narrowed type
    /// to be consistent with the input) from the positive-only `TypeGuard`.
    Guard {
        /// `true` for `TypeIs[T]`, `false` for `TypeGuard[T]`.
        type_is: bool,
        /// The narrowing target `T`, resolved through the same cascade.
        inner: Box<InferredType>,
    },
}

/// Represents a callable type's parameter and return type information.
#[derive(Debug, Clone, PartialEq)]
pub struct CallableInfo {
    /// Parameter types, positionally.
    ///
    /// A trailing [`GRADUAL_PARAMS`] marker means "and then any parameters":
    /// `Callable[..., R]` is `[…]`, a `ParamSpec` is `[…]`, and
    /// `Callable[Concatenate[int, P], R]` is `[int, …]` — the prefix is
    /// required, the tail unconstrained. An EMPTY list is therefore a callable
    /// that takes NO parameters (`Callable[[], R]`), which is what lets
    /// `Callable[Concatenate[int, P], str]` reject a zero-argument callable.
    pub param_types: Vec<InferredType>,
    /// Return type.
    pub return_type: Box<InferredType>,
}

/// The structural marker that ends an unconstrained parameter list. Shares the
/// spelling of the `tuple[X, ...]` terminator: both mean "the rest is not
/// pinned down here".
pub const GRADUAL_PARAMS: &str = "...";

/// A parameter list that is unconstrained past its (possibly empty) prefix.
#[must_use]
pub fn gradual_params(prefix: Vec<InferredType>) -> Vec<InferredType> {
    let mut params = prefix;
    params.push(InferredType::Named(GRADUAL_PARAMS.to_owned()));
    params
}

/// Split a parameter list into its required prefix and whether an
/// unconstrained tail follows.
#[must_use]
pub fn split_gradual(params: &[InferredType]) -> (&[InferredType], bool) {
    match params.split_last() {
        Some((InferredType::Named(marker), head)) if marker == GRADUAL_PARAMS => (head, true),
        _ => (params, false),
    }
}

/// Represents a literal value for literal type inference.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    /// Integer literal value
    Int(i64),
    /// String literal value
    Str(String),
    /// Float literal value
    Float(f64),
    /// Boolean literal value
    Bool(bool),
    /// Bytes literal value
    Bytes(Vec<u8>),
}

impl fmt::Display for InferredType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferredType::Int => write!(f, "int"),
            InferredType::Str => write!(f, "str"),
            InferredType::Float => write!(f, "float"),
            InferredType::Bool => write!(f, "bool"),
            InferredType::Bytes => write!(f, "bytes"),
            InferredType::None_ => write!(f, "None"),
            InferredType::Literal(lit) => write!(f, "Literal[{lit}]"),
            InferredType::List(elem_type) => write!(f, "list[{elem_type}]"),
            InferredType::Dict(key_type, value_type) => {
                write!(f, "dict[{key_type}, {value_type}]")
            }
            InferredType::Set(elem_type) => write!(f, "set[{elem_type}]"),
            InferredType::Tuple(elem_types) => {
                write!(f, "tuple[")?;
                for (i, elem_type) in elem_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem_type}")?;
                }
                write!(f, "]")
            }
            InferredType::Union(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{t}")?;
                }
                Ok(())
            }
            InferredType::Optional(inner) => write!(f, "Optional[{inner}]"),
            InferredType::Callable(info) => {
                write!(f, "Callable[[")?;
                for (i, param) in info.param_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, "], {}]", info.return_type)
            }
            InferredType::Generator(yield_type, send_type, return_type) => {
                write!(f, "Generator[{yield_type}, {send_type}, {return_type}]")
            }
            InferredType::Any => write!(f, "Any"),
            InferredType::Never => write!(f, "Never"),
            InferredType::Unknown => write!(f, "Unknown"),
            InferredType::LiteralString => write!(f, "LiteralString"),
            InferredType::Named(name) => write!(f, "{name}"),
            InferredType::TypeForm(inner) => match inner.as_ref() {
                InferredType::Any => write!(f, "TypeForm"),
                other => write!(f, "TypeForm[{other}]"),
            },
            InferredType::Guard { type_is, inner } => {
                let form = if *type_is { "TypeIs" } else { "TypeGuard" };
                write!(f, "{form}[{inner}]")
            }
        }
    }
}

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Int(val) => write!(f, "{val}"),
            LiteralValue::Str(val) => write!(f, "\"{val}\""),
            LiteralValue::Float(val) => write!(f, "{val}"),
            LiteralValue::Bool(val) => write!(f, "{val}"),
            LiteralValue::Bytes(val) => {
                let lossy = String::from_utf8_lossy(val);
                write!(f, "b\"{lossy}\"")
            }
        }
    }
}

impl InferredType {
    /// Creates a union of two types, flattening nested unions.
    ///
    /// Implements [TYPEINF-VARS-FLOW] — the join of types assigned across branches
    /// is their union. Also the `A | B` construction underpinning
    /// [TYPEINF-SUBTYPING-UNION]; `Never ∪ T = T` realises `Never` as the bottom
    /// type (identity element of union).
    #[must_use]
    pub fn union(a: InferredType, b: InferredType) -> InferredType {
        // Helper to flatten a type into the vector
        fn flatten_into(ty: InferredType, vec: &mut Vec<InferredType>) {
            match ty {
                InferredType::Union(mut inner_types) => {
                    for inner_ty in inner_types.drain(..) {
                        flatten_into(inner_ty, vec);
                    }
                }
                other => vec.push(other),
            }
        }

        // Handle Never specially: Never ∪ T = T
        if matches!(a, InferredType::Never) {
            return b;
        }
        if matches!(b, InferredType::Never) {
            return a;
        }

        // Flatten both sides into vectors of types
        let mut types = Vec::new();
        flatten_into(a, &mut types);
        flatten_into(b, &mut types);

        // Deduplicate types
        let mut deduplicated = Vec::new();
        for ty in types {
            if !deduplicated.contains(&ty) {
                deduplicated.push(ty);
            }
        }

        // If only one type remains, return it directly (not wrapped in Union)
        match deduplicated.len() {
            0 => InferredType::Never, // Should not happen due to Never handling above
            1 => match deduplicated.into_iter().next() {
                Some(ty) => ty,
                None => InferredType::Never, // len()==1 guarantees Some
            },
            _ => InferredType::Union(deduplicated),
        }
    }

    /// Returns true if this type is assignable to the other type.
    ///
    /// Implements [TYPEINF-SUBTYPING-IMPL] — the `InferredType`-level subtype check
    /// the spec calls `is_subtype_of()`. Covers the builtin numeric tower
    /// ([TYPEINF-SUBTYPING-NOMINAL]), union/optional/special-form relations
    /// ([TYPEINF-SUBTYPING-UNION]), container element assignability
    /// ([TYPEINF-SUBTYPING-GENERIC]) and callable variance
    /// ([TYPEINF-SUBTYPING-CALLABLE]). Nominal MRO and structural Protocol/TypedDict
    /// checks live in out-of-scope rule modules (see the consolidated map).
    #[must_use]
    pub fn is_assignable_to(&self, other: &InferredType) -> bool {
        match (self, other) {
            // Any is assignable to/from everything (PEP 484).
            // Unknown means we cannot determine the type — assume compatible to avoid false positives.
            // Never is the bottom type — assignable to everything.
            // Implements [TYPEINF-SPECIAL-ANY] (Any bidirectional) and
            // [TYPEINF-SPECIAL-NEVER] (Never <: everything, bottom type).
            (InferredType::Any | InferredType::Never | InferredType::Unknown, _)
            | (_, InferredType::Any | InferredType::Unknown) => true,
            // Same types are assignable
            (a, b) if a == b => true,
            // Int→float widening, Literal assignable to base type,
            // string types assignable to LiteralString, LiteralString to str.
            // Implements [TYPEINF-SUBTYPING-NOMINAL] — the hardcoded builtin tower
            // (bool <: int <: float) plus [TYPEINF-SPECIAL-LITERALSTRING]
            // (Literal[str]/LiteralString <-> str relations).
            (
                InferredType::Int | InferredType::Literal(LiteralValue::Float(_)),
                InferredType::Float,
            )
            // `bool` is a subclass of `int` (PEP 484), so plain `bool` widens
            // through the whole tower exactly like `Literal[bool]` does below.
            | (
                InferredType::Literal(LiteralValue::Int(_)) | InferredType::Bool,
                InferredType::Int | InferredType::Float,
            )
            | (
                InferredType::Literal(LiteralValue::Str(_)) | InferredType::LiteralString,
                InferredType::Str,
            )
            | (
                InferredType::Literal(LiteralValue::Bool(_)),
                InferredType::Bool | InferredType::Int,
            )
            | (InferredType::Literal(LiteralValue::Bytes(_)), InferredType::Bytes)
            | (
                InferredType::Literal(LiteralValue::Str(_)),
                InferredType::LiteralString,
            )
            // None is always assignable to Optional[T]
            | (InferredType::None_, InferredType::Optional(_)) => true,
            // `None` satisfies `Hashable` (it defines `__hash__`). Compared
            // case-insensitively: the [TYPEINF-ANNOTATION-RESOLUTION] cascade
            // keeps the ABC's real spelling, the legacy annotation parser it
            // replaces folded it to `Named("hashable")`.
            (InferredType::None_, InferredType::Named(name))
                if name.eq_ignore_ascii_case("hashable") =>
            {
                true
            }
            // `object` is the top type: every value IS an object, so it accepts
            // anything as a target. In the SOURCE position it stays as
            // permissive as the gradual `Any` it used to be modelled by —
            // narrowing an `object`-typed value to a concrete type is how most
            // `isinstance` code is written, and this level has no flow
            // information to tell a narrowed use from an unnarrowed one, so
            // rejecting it here would fire on spec-valid code.
            (_, InferredType::Named(name)) | (InferredType::Named(name), _)
                if name == "object" =>
            {
                true
            }
            // PEP 647/742 narrowing returns. Three distinct relations, and the
            // guard-to-guard one must be tested FIRST or the bool relations
            // below would make the two forms interchangeable:
            // * `TypeGuard[B] <: TypeGuard[A]` when `B <: A` — TypeGuard is
            //   covariant in its argument.
            // * `TypeIs[B] <: TypeIs[A]` only when `B` IS `A` — "Unlike
            //   TypeGuard, TypeIs is invariant in its argument type".
            // * Never across forms: "TypeIs and TypeGuard are not compatible
            //   with each other".
            (
                InferredType::Guard {
                    type_is: source_is,
                    inner: source_inner,
                },
                InferredType::Guard {
                    type_is: target_is,
                    inner: target_inner,
                },
            ) => {
                source_is == target_is
                    && if *target_is {
                        source_inner == target_inner
                    } else {
                        source_inner.is_assignable_to(target_inner)
                    }
            }
            // A guard VALUE is a `bool` ("in these contexts it is treated as a
            // subtype of bool"), so `Callable[..., TypeIs[int]]` satisfies
            // `Callable[..., bool]` but never `Callable[..., str]`.
            (InferredType::Guard { .. }, target) => InferredType::Bool.is_assignable_to(target),
            // Conversely a declared guard return is satisfied by any bool the
            // body actually produces — `def f(x: object) -> TypeIs[int]: return
            // False` is the canonical narrowing-function body, not a mismatch.
            (source, InferredType::Guard { .. }) => source.is_assignable_to(&InferredType::Bool),
            // Union on the LEFT decomposes before Optional-target unwrapping:
            // `A | None <: Optional[B]` must check each variant against the
            // whole `Optional[B]` (so the `None` arm can satisfy it), not
            // against the unwrapped `B`.
            // Implements [TYPEINF-SUBTYPING-UNION] — `A | B <: C` iff `A <: C` and
            // `B <: C`.
            (InferredType::Union(types), other) => types.iter().all(|t| t.is_assignable_to(other)),
            // Optional types are assignable to their non-optional counterparts.
            // Implements [TYPEINF-SUBTYPING-UNION] — Optional[T] = T | None handling.
            (InferredType::Optional(inner), other) => inner.is_assignable_to(other),
            (inner, InferredType::Optional(other)) => inner.is_assignable_to(other),
            // `A <: A | B` (a type is a subtype of any union containing it).
            (inner, InferredType::Union(types)) => types.iter().any(|t| inner.is_assignable_to(t)),
            // Mutable containers are invariant: each argument must be
            // compatible in both directions. This preserves gradual `Any` /
            // `Unknown` compatibility while rejecting one-way widening such
            // as `list[int]` -> `list[float]`.
            // Implements [TYPEINF-SUBTYPING-GENERIC] — list/set/dict are invariant
            // here (element types checked structurally, no cross-container matching).
            // List and Set cannot use or-patterns — that would incorrectly allow cross-matching.
            (InferredType::List(a), InferredType::List(b))
            | (InferredType::Set(a), InferredType::Set(b)) => invariantly_assignable(a, b),
            (InferredType::Dict(a_key, a_val), InferredType::Dict(b_key, b_val)) => {
                invariantly_assignable(a_key, b_key) && invariantly_assignable(a_val, b_val)
            }
            (InferredType::Tuple(a), InferredType::Tuple(b)) => {
                // Implements [TYPEINF-COLLECTIONS-TUPLES] — fixed-length positional
                // tuples plus the homogeneous `tuple[X, ...]` and PEP 646 unpacked
                // (`*tuple[...]`/`*Ts`) forms.
                // A target with an unpacked `*tuple[...]` / `*Ts` segment (PEP 646)
                // needs prefix/middle/suffix matching, not positional equality.
                if b.iter().any(is_unpacked_tuple_elem) {
                    return tuple_assignable_with_star(a, b);
                }
                match (homogeneous_tuple_elem(a), homogeneous_tuple_elem(b)) {
                    // Target `tuple[X, ...]` (PEP 484 homogeneous variable-length):
                    // a source `tuple[Y, ...]` matches when `Y` is assignable to `X`.
                    (Some(a_elem), Some(b_elem)) => a_elem.is_assignable_to(b_elem),
                    // Target `tuple[X, ...]`: every element of a fixed-length source
                    // tuple must be assignable to `X` (empty tuple is vacuously valid).
                    (None, Some(b_elem)) => a.iter().all(|elem| elem.is_assignable_to(b_elem)),
                    // Target is fixed-length: a variable-length source cannot
                    // satisfy it — EXCEPT `tuple[Any, ...]` / `tuple[Unknown, ...]`,
                    // which are bidirectionally compatible with any tuple (PEP 484
                    // gradual typing).  Unresolved `Named` elements (e.g. types from
                    // unresolvable imports) are gradual too, not provable mismatches.
                    (Some(source_elem), None) => {
                        matches!(
                            source_elem,
                            InferredType::Any | InferredType::Unknown | InferredType::Named(_)
                        )
                    }
                    // Both fixed-length: require equal arity and positional assignability.
                    (None, None) => {
                        a.len() == b.len()
                            && a.iter()
                                .zip(b.iter())
                                .all(|(a_elem, b_elem)| a_elem.is_assignable_to(b_elem))
                    }
                }
            }
            // Callable type assignability.
            // Implements [TYPEINF-SUBTYPING-CALLABLE] — return type covariant
            // (source return <: target return), parameters contravariant
            // (target param <: source param), `...`/empty params gradual.
            (InferredType::Callable(a), InferredType::Callable(b)) => callable_assignable(a, b),
            (a @ InferredType::Generator(..), b @ InferredType::Generator(..)) => {
                generator_assignable(a, b)
            }
            // TypeForm covariance: TypeForm[S] is assignable to TypeForm[T] if S is assignable to T.
            (InferredType::TypeForm(inner_a), InferredType::TypeForm(inner_b)) => {
                inner_a.is_assignable_to(inner_b)
            }
            // Named types with the same base name (before `[`) are assumed compatible.
            // Without full generic variance analysis we cannot determine if
            // `Foo[int]` is assignable to `Foo[float]`, so we avoid false positives.
            (InferredType::Named(a_name), InferredType::Named(b_name)) => {
                let a_base = a_name.split('[').next().unwrap_or(a_name);
                let b_base = b_name.split('[').next().unwrap_or(b_name);
                a_base == b_base
            }
            // Ellipsis (`...`) parsed as Named is compatible when it appears
            // inside Callable parameter lists (e.g. `Callable[..., T]`).
            // For tuple annotations, `...` has special semantics that need
            // structural checking, so we don't treat it as universally compatible.
            _ => false,
        }
    }
}

fn invariantly_assignable(left: &InferredType, right: &InferredType) -> bool {
    left.is_assignable_to(right) && right.is_assignable_to(left)
}

/// Callable subtyping: returns covariant, parameters contravariant.
///
/// Implements [TYPEINF-SUBTYPING-CALLABLE]. An `Unknown` source return (a
/// lambda whose body we could not infer) is only accepted against a gradual
/// target — claiming compatibility with a KNOWN target return would assert
/// something unverified.
fn callable_assignable(source: &CallableInfo, target: &CallableInfo) -> bool {
    let target_return_is_gradual = matches!(
        &*target.return_type,
        InferredType::Any | InferredType::Unknown
    );
    if matches!(&*source.return_type, InferredType::Unknown) && !target_return_is_gradual {
        return false;
    }
    if !source.return_type.is_assignable_to(&target.return_type) {
        return false;
    }
    callable_params_assignable(&source.param_types, &target.param_types)
}

/// Parameter-list half of [TYPEINF-SUBTYPING-CALLABLE].
///
/// Positions are contravariant: the target's parameter must be acceptable to
/// the source. A source that requires FEWER positions than the target is fine —
/// its trailing positions are satisfiable by defaults — but never more.
///
/// A gradual tail ([`GRADUAL_PARAMS`]) relaxes only what follows it. A gradual
/// SOURCE accepts any call, so it satisfies every target. A gradual TARGET
/// (`Callable[Concatenate[int, P], R]`) still pins its prefix: the source must
/// be able to receive those leading arguments, which is exactly why a
/// zero-parameter callable does not satisfy it.
fn callable_params_assignable(source: &[InferredType], target: &[InferredType]) -> bool {
    let (source_prefix, source_gradual) = split_gradual(source);
    let (target_prefix, target_gradual) = split_gradual(target);
    if source_gradual {
        return true;
    }
    if target_gradual && source_prefix.len() < target_prefix.len() {
        return false;
    }
    if !target_gradual && source_prefix.len() > target_prefix.len() {
        return false;
    }
    source_prefix
        .iter()
        .zip(target_prefix.iter())
        .all(|(source_param, target_param)| target_param.is_assignable_to(source_param))
}

/// Generator yield/return positions are covariant; the value sent back into
/// the suspended generator is contravariant.
fn generator_assignable(left: &InferredType, right: &InferredType) -> bool {
    let (
        InferredType::Generator(left_yield, left_send, left_return),
        InferredType::Generator(right_yield, right_send, right_return),
    ) = (left, right)
    else {
        return false;
    };
    left_yield.is_assignable_to(right_yield)
        && right_send.is_assignable_to(left_send)
        && left_return.is_assignable_to(right_return)
}

pub(crate) use crate::types_star_tuples::{
    homogeneous_tuple_elem, is_unpacked_tuple_elem, tuple_assignable_with_star,
};
