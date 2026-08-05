//! Implements [TYPEINF-TARGET-TYPELEVEL] — the type-level term language.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL
//!
//! [`TypeTerm`] is the object language of the normalization-by-evaluation
//! engine: ground types, constructors, alias applications, **kind
//! `Type → Type` operator values** (mapped types), higher-order application,
//! and **conditional types** as guarded rewrites on assignability.
//! [`AliasEnv`] is the definition environment with the acceptance-checked
//! [`AliasEnv::insert`] front door and the opt-in
//! [`AliasEnv::insert_undecidable`] escape hatch (GHC's
//! `UndecidableInstances` analogue — fuel/depth bounds remain the safety
//! net).

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::types::InferredType;

use super::accept::{classify, Acceptance};

/// The kind of a type-level value ([TYPEINF-TARGET-TYPELEVEL]).
///
/// Ground types and fully-applied constructors have kind [`Kind::Type`]; an
/// alias with `n ≥ 1` parameters used *unapplied* is an operator of kind
/// `Type → … → Type` ([`Kind::Operator`]) — the mapped-type representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A proper type (`*`): inhabitable, assignable, a whnf value.
    Type,
    /// An `arity`-ary type operator (`Type → … → Type`, `arity ≥ 1`).
    Operator {
        /// Number of type arguments the operator expects.
        arity: usize,
    },
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Type => write!(f, "Type"),
            Kind::Operator { arity } => {
                for _ in 0..*arity {
                    write!(f, "Type → ")?;
                }
                write!(f, "Type")
            }
        }
    }
}

/// A conditional type: `then_arm if scrutinee <: against else else_arm` —
/// PEP 827's `IsAssignable`-guarded rewrite, evaluated **lazily**
/// (call-by-need): only the taken arm is ever normalized, so a divergent
/// untaken arm cannot make the whole conditional diverge.
#[derive(Debug, Clone, PartialEq)]
pub struct CondTerm {
    /// The type being tested (forced to whnf to decide the rewrite).
    pub scrutinee: TypeTerm,
    /// The pattern the scrutinee is tested against (forced to whnf).
    pub against: TypeTerm,
    /// Arm taken when `scrutinee <: against` (lazy).
    pub then_arm: TypeTerm,
    /// Arm taken otherwise (lazy).
    pub else_arm: TypeTerm,
}

/// A type-level term.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeTerm {
    /// A ground type — already a value.
    Ground(InferredType),
    /// A reference to an alias, possibly applied: `Pair[int]`, `Json`.
    Alias(String, Vec<TypeTerm>),
    /// A reference to the enclosing alias's parameter by index.
    Param(usize),
    /// An alias *used unapplied* as a first-class operator value of kind
    /// `Type → … → Type` — the mapped-type representation. `Op("Pair")`
    /// can be passed as an argument and applied later via [`TypeTerm::Apply`].
    Op(String),
    /// Higher-order application: apply an operator-valued head (an
    /// [`TypeTerm::Op`], or a [`TypeTerm::Param`] bound to one) to arguments.
    Apply(Box<TypeTerm>, Vec<TypeTerm>),
    /// A conditional type — a guarded rewrite on assignability
    /// ([`CondTerm`]), evaluated call-by-need.
    Cond(Box<CondTerm>),
    /// `list[T]` at the type level (constructor — a whnf head).
    List(Box<TypeTerm>),
    /// `set[T]` / `frozenset[T]` at the type level.
    Set(Box<TypeTerm>),
    /// `dict[K, V]` at the type level.
    Dict(Box<TypeTerm>, Box<TypeTerm>),
    /// `T | U` at the type level.
    Union(Vec<TypeTerm>),
    /// `tuple[T, ..]` at the type level.
    Tuple(Vec<TypeTerm>),
    /// Any other named generic constructor: `Sequence[T]`, `Callable[..]`,
    /// `Mapping[K, V]` — a whnf head whose arguments stay lazy.
    Named(String, Vec<TypeTerm>),
}

/// One alias definition: `type Name[P0, P1, ..] = body`.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasDef {
    /// Number of type parameters.
    pub arity: usize,
    /// The right-hand side, with [`TypeTerm::Param`] for parameters.
    pub body: TypeTerm,
}

impl AliasDef {
    /// The kind of this definition: `Type` when nullary, else the
    /// `arity`-ary operator kind — mapped types ARE `Type → Type` operators.
    #[must_use]
    pub fn kind(&self) -> Kind {
        if self.arity == 0 {
            Kind::Type
        } else {
            Kind::Operator { arity: self.arity }
        }
    }
}

/// The alias environment (one module's `type` statements).
///
/// Mutual recursion note: acceptance is a *per-definition* condition, so a
/// bare mutual cycle (`type A = B` / `type B = A`) inserts fine and is
/// handled **gradually** at evaluation time — fuel/depth exhaust and the
/// result projects to `Unknown`, never an invented error. Diagnosing such
/// cycles is the checker rule's job (`generics_syntax_scoping`), not the
/// engine's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AliasEnv {
    aliases: HashMap<String, AliasDef>,
    /// Names admitted through the [`AliasEnv::insert_undecidable`] escape
    /// hatch — recorded so tooling can surface that they rely on
    /// fuel-bounded evaluation alone.
    undecidable: HashSet<String>,
}

impl AliasEnv {
    /// Register an alias behind the acceptance conditions
    /// ([`classify`]): rejects (returns `false`, leaving the environment
    /// unchanged) definitions whose recursion is unguarded (`type X = X`,
    /// union arms included) or non-regular (a self-application whose
    /// arguments grow — the Paterson/Coverage analogue).
    pub fn insert(&mut self, name: &str, def: AliasDef) -> bool {
        if classify(name, &def) != Acceptance::Accepted {
            return false;
        }
        let _ = self.aliases.insert(name.to_owned(), def);
        true
    }

    /// The opt-in "undecidable" escape hatch: register `def` **without**
    /// the static acceptance conditions, GHC-`UndecidableInstances`-style.
    /// Termination then rests entirely on the evaluator's fuel/depth
    /// bounds, whose exhaustion projects to the gradual `Unknown`
    /// ([TYPEINF-TARGET-GRADUAL]) — never an invented error.
    pub fn insert_undecidable(&mut self, name: &str, def: AliasDef) {
        let _ = self.undecidable.insert(name.to_owned());
        let _ = self.aliases.insert(name.to_owned(), def);
    }

    /// Look up an alias.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AliasDef> {
        self.aliases.get(name)
    }

    /// The kind of a registered alias, if any.
    #[must_use]
    pub fn kind_of(&self, name: &str) -> Option<Kind> {
        self.aliases.get(name).map(AliasDef::kind)
    }

    /// Was `name` admitted through the undecidable escape hatch?
    #[must_use]
    pub fn is_undecidable(&self, name: &str) -> bool {
        self.undecidable.contains(name)
    }

    /// Iterate over registered alias names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.aliases.keys().map(String::as_str)
    }

    /// `true` when no aliases are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kinds: nullary aliases are `Type`; parameterised aliases are
    /// operators — mapped types represented at kind `Type → Type`.
    #[test]
    fn alias_kinds_reflect_arity() {
        let nullary = AliasDef {
            arity: 0,
            body: TypeTerm::Ground(InferredType::Int),
        };
        let unary = AliasDef {
            arity: 1,
            body: TypeTerm::List(Box::new(TypeTerm::Param(0))),
        };
        assert_eq!(nullary.kind(), Kind::Type);
        assert_eq!(unary.kind(), Kind::Operator { arity: 1 });
        assert_eq!(nullary.kind().to_string(), "Type");
        assert_eq!(unary.kind().to_string(), "Type → Type");
        assert_eq!(
            (Kind::Operator { arity: 2 }).to_string(),
            "Type → Type → Type"
        );
    }

    /// The escape hatch admits what `insert` rejects, and records it.
    #[test]
    fn undecidable_escape_hatch_bypasses_acceptance() {
        let unguarded = AliasDef {
            arity: 0,
            body: TypeTerm::Alias("X".to_owned(), Vec::new()),
        };
        let mut env = AliasEnv::default();
        assert!(!env.insert("X", unguarded.clone()));
        assert!(env.get("X").is_none());

        env.insert_undecidable("X", unguarded);
        assert!(env.get("X").is_some());
        assert!(env.is_undecidable("X"));
        assert!(!env.is_undecidable("Y"));
    }
}
