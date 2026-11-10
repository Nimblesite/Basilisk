//! Implements [TYPEINF-TARGET-CONSTRAINTS] (declared-generics layer). See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CONSTRAINTS
//!
//! User-declared generic parameters and their deterministic solver.
//!
//! [`super::tyvar::TyVarStore`] handles the engine's *anonymous* inference
//! variables. This module handles *declared* variables — `TypeVar`,
//! `ParamSpec`, `TypeVarTuple` — which carry declaration-site facts an
//! anonymous variable never has: an upper `bound=`, a constrained value set
//! (`TypeVar('T', int, str)`), and a PEP 696 `default=`. Evidence accumulates
//! as explicit lower/upper bounds (expected-return propagation records an
//! upper bound), then [`GenericEnv::resolve`] answers **deterministically**:
//! evidence supporting several incomparable answers is reported as
//! [`Resolution::Ambiguous`], never guessed. Ground assignability delegates
//! to the single authority, [`InferredType::is_assignable_to`], so the
//! gradual `Any`/`Unknown` posture ([TYPEINF-TARGET-GRADUAL]) is inherited,
//! not re-implemented.
//!
//! # No production caller yet — deliberately
//!
//! Nothing in this module is consumed by a rule, and that is the order
//! [NARROWPLAN-CONSTRAINTS] asks for: "Cover constrained/bound `TypeVar`s,
//! PEP 696 defaults, `ParamSpec`, and `TypeVarTuple` interactions **before**
//! wiring the solver into rule decisions." The solver plus its 21-case
//! interaction suite (`tests/generic_constraints_tests.rs`) is that coverage;
//! the wiring is Integration-stage work ([NARROWPLAN-INTEGRATION]). Wiring it
//! earlier would put an unproven solver behind live diagnostics, risking the
//! zero-false-positive pristine-fixture regression guard. That guard is only
//! one evidence layer; mutation and independently derived cases are required.
//!
//! **Lint posture — do not "fix" this by narrowing visibility.** The
//! workspace denies `dead_code`. This module satisfies it because it is
//! `pub mod generics` re-exported through `bidir`, itself `pub mod bidir` at
//! the crate root, so every item is reachable from outside the crate and
//! therefore live. Demoting the module, the re-export, or any item to
//! `pub(crate)` before the Integration-stage wiring lands would make
//! `dead_code` fire on a deliberate placeholder — and the fix for THAT is to
//! wire the rules up, never to add an `#[allow]`/`#[expect]`.

use crate::types::InferredType;

/// What kind of generic parameter was declared.
#[derive(Debug, Clone, PartialEq)]
pub enum DeclaredVarKind {
    /// `TypeVar('T')`, `TypeVar('T', bound=B)`, or `TypeVar('T', A, B, …)`.
    ///
    /// A non-empty `constraints` list means the variable solves to exactly
    /// one of the listed types, never a join of them (typing spec,
    /// "Constrained type variables").
    TypeVar {
        /// Declared upper bound (`bound=`), if any.
        bound: Option<InferredType>,
        /// Declared value constraints; empty for an unconstrained variable.
        constraints: Vec<InferredType>,
    },
    /// `ParamSpec('P')` — solves to a captured parameter list.
    ParamSpec,
    /// `TypeVarTuple('Ts')` — solves to a captured sequence of types.
    TypeVarTuple,
}

/// A PEP 696 declared default, shaped by the variable's kind.
#[derive(Debug, Clone, PartialEq)]
pub enum VarDefault {
    /// `TypeVar('T', default=int)`.
    Type(InferredType),
    /// `ParamSpec('P', default=[int, str])`; `None` is the gradual `...`.
    Params(Option<Vec<InferredType>>),
    /// `TypeVarTuple('Ts', default=Unpack[tuple[int, str]])`.
    Elements(Vec<InferredType>),
}

/// One declared generic parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredVar {
    /// The declared name (`T`, `P`, `Ts`).
    pub name: String,
    /// `TypeVar` / `ParamSpec` / `TypeVarTuple`, with declaration-site facts.
    pub kind: DeclaredVarKind,
    /// PEP 696 default, used only when no evidence was collected.
    pub default: Option<VarDefault>,
}

/// A deterministic solved value, shaped by the variable's kind.
#[derive(Debug, Clone, PartialEq)]
pub enum SolvedValue {
    /// A single type — the `TypeVar` case.
    Type(InferredType),
    /// A parameter list — the `ParamSpec` case (`None` is the gradual `...`).
    Params(Option<Vec<InferredType>>),
    /// A fixed sequence of types — the `TypeVarTuple` case.
    Elements(Vec<InferredType>),
}

/// The outcome of solving one declared variable.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Exactly one defensible answer, derived from evidence.
    Solved(SolvedValue),
    /// No evidence; the PEP 696 default answered.
    DefaultUsed(SolvedValue),
    /// No evidence and no default — reported, never guessed
    /// ([TYPEINF-EXCEEDS-NOUNKNOWN]).
    Unsolved,
    /// Evidence supports several incomparable answers (empty `candidates`
    /// means the evidence itself was of conflicting kinds).
    Ambiguous {
        /// Every defensible answer, in declaration/collection order.
        candidates: Vec<SolvedValue>,
    },
    /// Evidence contradicts the declaration or itself.
    Unsatisfiable {
        /// What the evidence produced.
        actual: InferredType,
        /// What the declaration or context demanded.
        expected: InferredType,
    },
}

/// One variable's declaration plus accumulated evidence.
#[derive(Debug, Clone)]
struct VarState {
    decl: DeclaredVar,
    /// Types that flowed *into* the variable (call arguments).
    lowers: Vec<InferredType>,
    /// Types demanded *of* the variable (declared bound checks aside,
    /// expected-return propagation lands here).
    uppers: Vec<InferredType>,
    /// Captured parameter lists for a `ParamSpec` (`None` is gradual `...`).
    param_captures: Vec<Option<Vec<InferredType>>>,
    /// Captured element sequences for a `TypeVarTuple`.
    element_captures: Vec<Vec<InferredType>>,
    /// Evidence arrived that only a different kind could use.
    kind_conflict: bool,
}

/// Identifier of a declared variable inside one [`GenericEnv`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericVarId(usize);

/// Declared variables plus evidence for one inference site (one call, one
/// generic class instantiation).
#[derive(Debug, Clone, Default)]
pub struct GenericEnv {
    vars: Vec<VarState>,
}

impl GenericEnv {
    /// Declare a variable, returning its id.
    pub fn declare(&mut self, decl: DeclaredVar) -> GenericVarId {
        let id = self.vars.len();
        self.vars.push(VarState {
            decl,
            lowers: Vec::new(),
            uppers: Vec::new(),
            param_captures: Vec::new(),
            element_captures: Vec::new(),
            kind_conflict: false,
        });
        GenericVarId(id)
    }

    /// Look a declared variable up by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<GenericVarId> {
        self.vars
            .iter()
            .position(|v| v.decl.name == name)
            .map(GenericVarId)
    }

    /// Record `ty <: var` — an argument type flowed in. Duplicates drop, so
    /// repeated identical evidence cannot change the answer.
    pub fn add_lower(&mut self, id: GenericVarId, ty: InferredType) {
        if let Some(state) = self.vars.get_mut(id.0) {
            match state.decl.kind {
                DeclaredVarKind::TypeVar { .. } if !state.lowers.contains(&ty) => {
                    state.lowers.push(ty);
                }
                DeclaredVarKind::TypeVar { .. } => {}
                _ => state.kind_conflict = true,
            }
        }
    }

    /// Record `var <: ty` — the context demanded this (expected-return
    /// propagation records the expected type here). Duplicates drop.
    pub fn add_upper(&mut self, id: GenericVarId, ty: InferredType) {
        if let Some(state) = self.vars.get_mut(id.0) {
            match state.decl.kind {
                DeclaredVarKind::TypeVar { .. } if !state.uppers.contains(&ty) => {
                    state.uppers.push(ty);
                }
                DeclaredVarKind::TypeVar { .. } => {}
                _ => state.kind_conflict = true,
            }
        }
    }

    /// Record a captured parameter list for a `ParamSpec`.
    pub fn add_param_capture(&mut self, id: GenericVarId, params: Option<Vec<InferredType>>) {
        if let Some(state) = self.vars.get_mut(id.0) {
            match state.decl.kind {
                DeclaredVarKind::ParamSpec if !state.param_captures.contains(&params) => {
                    state.param_captures.push(params);
                }
                DeclaredVarKind::ParamSpec => {}
                _ => state.kind_conflict = true,
            }
        }
    }

    /// Record a captured element sequence for a `TypeVarTuple`.
    pub fn add_element_capture(&mut self, id: GenericVarId, elements: Vec<InferredType>) {
        if let Some(state) = self.vars.get_mut(id.0) {
            match state.decl.kind {
                DeclaredVarKind::TypeVarTuple if !state.element_captures.contains(&elements) => {
                    state.element_captures.push(elements);
                }
                DeclaredVarKind::TypeVarTuple => {}
                _ => state.kind_conflict = true,
            }
        }
    }

    /// Solve one variable. Deterministic: the answer depends only on the
    /// declaration and the deduplicated evidence, in collection order.
    #[must_use]
    pub fn resolve(&self, id: GenericVarId) -> Resolution {
        let Some(state) = self.vars.get(id.0) else {
            return Resolution::Unsolved;
        };
        if state.kind_conflict {
            return Resolution::Ambiguous {
                candidates: Vec::new(),
            };
        }
        match &state.decl.kind {
            DeclaredVarKind::TypeVar { bound, constraints } if constraints.is_empty() => {
                solve_typevar(state, bound.as_ref())
            }
            DeclaredVarKind::TypeVar { constraints, .. } => solve_constrained(state, constraints),
            DeclaredVarKind::ParamSpec => solve_paramspec(state),
            DeclaredVarKind::TypeVarTuple => solve_typevartuple(state),
        }
    }

    /// Solve every declared variable, in declaration order.
    #[must_use]
    pub fn resolve_all(&self) -> Vec<(String, Resolution)> {
        (0..self.vars.len())
            .map(|index| {
                let name = self
                    .vars
                    .get(index)
                    .map(|v| v.decl.name.clone())
                    .unwrap_or_default();
                (name, self.resolve(GenericVarId(index)))
            })
            .collect()
    }
}

/// An unconstrained (possibly `bound=`-ed) `TypeVar`.
///
/// The join of the lower bounds answers, kept precise (deferred
/// generalization: a lone `Literal[1]` stays `Literal[1]`). The join must
/// satisfy the declared bound and every demanded upper.
fn solve_typevar(state: &VarState, bound: Option<&InferredType>) -> Resolution {
    if state.lowers.is_empty() {
        return solve_typevar_without_lowers(state, bound);
    }
    let join = state
        .lowers
        .iter()
        .cloned()
        .fold(InferredType::Never, InferredType::union);
    if let Some(bound) = bound {
        if !join.is_assignable_to(bound) {
            return unsatisfiable(join, bound.clone());
        }
    }
    match state.uppers.iter().find(|u| !join.is_assignable_to(u)) {
        Some(upper) => unsatisfiable(join, upper.clone()),
        None => Resolution::Solved(SolvedValue::Type(join)),
    }
}

/// No arguments flowed in: the demanded type answers if it is unique,
/// then the PEP 696 default, and otherwise the variable stays unsolved.
fn solve_typevar_without_lowers(state: &VarState, bound: Option<&InferredType>) -> Resolution {
    match state.uppers.as_slice() {
        [] => default_or_unsolved(state),
        [upper] => match bound {
            Some(bound) if !upper.is_assignable_to(bound) => {
                unsatisfiable(upper.clone(), bound.clone())
            }
            _ => Resolution::Solved(SolvedValue::Type(upper.clone())),
        },
        uppers => Resolution::Ambiguous {
            candidates: uppers
                .iter()
                .map(|u| SolvedValue::Type(u.clone()))
                .collect(),
        },
    }
}

/// A value-constrained `TypeVar('T', A, B, …)`: the answer is exactly one
/// listed constraint, never a join of them (typing spec, "Constrained type
/// variables"). Every lower must select the SAME constraint; evidence
/// selecting different constraints is ambiguous, not merged.
fn solve_constrained(state: &VarState, constraints: &[InferredType]) -> Resolution {
    if state.lowers.is_empty() && state.uppers.is_empty() {
        return default_or_unsolved(state);
    }
    let mut selected: Option<usize> = None;
    for lower in &state.lowers {
        let Some(index) = constraints.iter().position(|c| lower.is_assignable_to(c)) else {
            let expected = constraints
                .iter()
                .cloned()
                .fold(InferredType::Never, InferredType::union);
            return unsatisfiable(lower.clone(), expected);
        };
        if selected.is_some_and(|prior| prior != index) {
            return Resolution::Ambiguous {
                candidates: constraints
                    .iter()
                    .map(|c| SolvedValue::Type(c.clone()))
                    .collect(),
            };
        }
        selected = Some(index);
    }
    constrained_answer(state, constraints, selected)
}

/// Finish a constrained solve: verify the selected constraint against the
/// demanded uppers, or select by uppers alone when no argument flowed in.
fn constrained_answer(
    state: &VarState,
    constraints: &[InferredType],
    selected: Option<usize>,
) -> Resolution {
    let satisfies_uppers =
        |candidate: &InferredType| state.uppers.iter().all(|u| candidate.is_assignable_to(u));
    if let Some(candidate) = selected.and_then(|index| constraints.get(index)) {
        return if satisfies_uppers(candidate) {
            Resolution::Solved(SolvedValue::Type(candidate.clone()))
        } else {
            let expected = state
                .uppers
                .iter()
                .cloned()
                .fold(InferredType::Never, InferredType::union);
            unsatisfiable(candidate.clone(), expected)
        };
    }
    let viable: Vec<&InferredType> = constraints.iter().filter(|c| satisfies_uppers(c)).collect();
    match viable.as_slice() {
        [] => {
            let actual = constraints
                .iter()
                .cloned()
                .fold(InferredType::Never, InferredType::union);
            let expected = state
                .uppers
                .iter()
                .cloned()
                .fold(InferredType::Never, InferredType::union);
            unsatisfiable(actual, expected)
        }
        [one] => Resolution::Solved(SolvedValue::Type((*one).clone())),
        several => Resolution::Ambiguous {
            candidates: several
                .iter()
                .map(|c| SolvedValue::Type((*c).clone()))
                .collect(),
        },
    }
}

/// A `ParamSpec`: one distinct capture answers; conflicting captures are
/// reported, never merged (parameter lists have no meaningful join).
fn solve_paramspec(state: &VarState) -> Resolution {
    match state.param_captures.as_slice() {
        [] => default_or_unsolved(state),
        [single] => Resolution::Solved(SolvedValue::Params(single.clone())),
        several => Resolution::Ambiguous {
            candidates: several
                .iter()
                .map(|p| SolvedValue::Params(p.clone()))
                .collect(),
        },
    }
}

/// A `TypeVarTuple`: captures of one length join elementwise; captures of
/// different lengths have no defensible common answer.
fn solve_typevartuple(state: &VarState) -> Resolution {
    let Some(first) = state.element_captures.first() else {
        return default_or_unsolved(state);
    };
    if state
        .element_captures
        .iter()
        .any(|capture| capture.len() != first.len())
    {
        return Resolution::Ambiguous {
            candidates: state
                .element_captures
                .iter()
                .map(|c| SolvedValue::Elements(c.clone()))
                .collect(),
        };
    }
    let joined = (0..first.len())
        .map(|position| {
            state
                .element_captures
                .iter()
                .filter_map(|capture| capture.get(position).cloned())
                .fold(InferredType::Never, InferredType::union)
        })
        .collect();
    Resolution::Solved(SolvedValue::Elements(joined))
}

/// No evidence at all: the PEP 696 default answers, else honestly unsolved.
fn default_or_unsolved(state: &VarState) -> Resolution {
    match &state.decl.default {
        Some(VarDefault::Type(ty)) => Resolution::DefaultUsed(SolvedValue::Type(ty.clone())),
        Some(VarDefault::Params(params)) => {
            Resolution::DefaultUsed(SolvedValue::Params(params.clone()))
        }
        Some(VarDefault::Elements(elements)) => {
            Resolution::DefaultUsed(SolvedValue::Elements(elements.clone()))
        }
        None => Resolution::Unsolved,
    }
}

/// Shorthand for the mismatch outcome.
fn unsatisfiable(actual: InferredType, expected: InferredType) -> Resolution {
    Resolution::Unsatisfiable { actual, expected }
}
