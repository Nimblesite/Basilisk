//! Implements [TYPEINF-TARGET-CONSTRAINTS]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS
//! Subtype constraints produced by the generation pass.
//!
//! The bidirectional traversal ([`super::engine`]) never decides subtyping on
//! its own: it only *records* `sub <: sup` obligations here, and the separate
//! solver ([`super::solve`]) discharges them — the Pottier–Rémy two-stage
//! split the spec mandates.

use ruff_text_size::TextRange;

use super::ty::Ty;

/// Why a constraint exists — drives the eventual diagnostic wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintReason {
    /// An element flowing into a list/set literal's element type.
    CollectionElement,
    /// A key flowing into a dict literal's key type.
    DictKey,
    /// A value flowing into a dict literal's value type.
    DictValue,
    /// A call argument flowing into the callee's declared parameter.
    CallArgument,
    /// A callee's return flowing into the expression's expected type.
    CallReturn,
    /// A lambda body flowing into the expected return type.
    LambdaBody,
    /// A comprehension's element flowing into the expected element type.
    ComprehensionElement,
    /// An expression checked directly against an expected (annotated) type.
    ExpectedType,
    /// A walrus (`:=`) value flowing into the expression's expected type.
    WalrusValue,
}

/// One `sub <: sup` obligation with the source location that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    /// The type that must be assignable…
    pub sub: Ty,
    /// …to this type.
    pub sup: Ty,
    /// Source range of the expression that generated the obligation.
    pub range: TextRange,
    /// Why the obligation exists.
    pub reason: ConstraintReason,
}

/// The ordered set of obligations one generation pass produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConstraintSet {
    constraints: Vec<Constraint>,
}

impl ConstraintSet {
    /// Record one `sub <: sup` obligation.
    pub fn push(&mut self, sub: Ty, sup: Ty, range: TextRange, reason: ConstraintReason) {
        self.constraints.push(Constraint {
            sub,
            sup,
            range,
            reason,
        });
    }

    /// The recorded obligations, in generation order.
    #[must_use]
    pub fn as_slice(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Number of recorded obligations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Whether nothing has been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Consume the set, yielding the obligations for the solver.
    #[must_use]
    pub fn into_vec(self) -> Vec<Constraint> {
        self.constraints
    }
}
