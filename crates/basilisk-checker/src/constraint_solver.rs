//! `TypeVar` constraint solver for generic function calls.
//!
//! Implements bidirectional `TypeVar` constraint solving per
//! `CHECKER-TYPE-INFERENCE-SPEC.md` §6.1–6.5.

use std::collections::HashMap;

use crate::types::InferredType;

/// A constraint on a `TypeVar`.
#[derive(Debug, Clone)]
pub enum TypeConstraint {
    /// The `TypeVar` must be a supertype of this (lower bound).
    LowerBound(InferredType),
    /// The `TypeVar` must be a subtype of this (upper bound / bound).
    UpperBound(InferredType),
    /// The `TypeVar` must be exactly one of these (constrained `TypeVar`).
    OneOf(Vec<InferredType>),
}

/// Solves `TypeVar` constraints from generic function calls.
///
/// Collects constraints from argument types against `TypeVar`-bearing
/// parameter types, then solves to produce a concrete type for each `TypeVar`.
#[derive(Debug, Clone)]
pub struct ConstraintSolver {
    /// `TypeVar` name → collected constraints.
    constraints: HashMap<String, Vec<TypeConstraint>>,
    /// `TypeVar` name → default type (PEP 696).
    defaults: HashMap<String, InferredType>,
}

/// Error when constraint solving fails.
#[derive(Debug, Clone)]
pub struct SolveError {
    /// The `TypeVar` that couldn't be solved.
    pub typevar: String,
    /// Description of why solving failed.
    pub reason: String,
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot solve `TypeVar` `{}`: {}",
            self.typevar, self.reason
        )
    }
}

impl ConstraintSolver {
    /// Create a new empty solver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            constraints: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------
    // 3a. Constraint collection from call arguments
    // -----------------------------------------------------------------

    /// Add a lower bound constraint: `TypeVar` must be a supertype of `ty`.
    ///
    /// This is the most common constraint — when an argument of type `ty`
    /// is passed to a parameter of type `T`, we know `T >= ty`.
    pub fn add_lower_bound(&mut self, typevar: &str, ty: InferredType) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(TypeConstraint::LowerBound(ty));
    }

    /// Add an upper bound constraint: `TypeVar` must be a subtype of `ty`.
    ///
    /// Used for bound `TypeVar`s (`T = TypeVar("T", bound=Comparable)`).
    pub fn add_upper_bound(&mut self, typevar: &str, ty: InferredType) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(TypeConstraint::UpperBound(ty));
    }

    /// Add a constrained `TypeVar`: must be exactly one of the given types.
    ///
    /// Used for `AnyStr = TypeVar("AnyStr", str, bytes)`.
    pub fn add_one_of(&mut self, typevar: &str, options: Vec<InferredType>) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(TypeConstraint::OneOf(options));
    }

    // -----------------------------------------------------------------
    // 3c. Bidirectional constraint from expected return type
    // -----------------------------------------------------------------

    /// Add a constraint from the expected return type (bidirectional).
    ///
    /// When the caller expects a specific return type, this can provide
    /// additional information to disambiguate `TypeVar` solutions.
    pub fn add_return_constraint(&mut self, typevar: &str, expected_return: InferredType) {
        self.add_lower_bound(typevar, expected_return);
    }

    // -----------------------------------------------------------------
    // 3f. `TypeVar` defaults (PEP 696, §6.5)
    // -----------------------------------------------------------------

    /// Register a default type for a `TypeVar`.
    pub fn set_default(&mut self, typevar: &str, default: InferredType) {
        let _ = self.defaults.insert(typevar.to_owned(), default);
    }

    // -----------------------------------------------------------------
    // 3b. Lower-bound join / upper-bound meet solving
    // -----------------------------------------------------------------

    /// Solve all constraints and return the resolved types.
    ///
    /// # Errors
    ///
    /// Returns `SolveError` if any `TypeVar` has contradictory constraints.
    pub fn solve(&self) -> Result<HashMap<String, InferredType>, SolveError> {
        let mut solutions = HashMap::new();

        for (typevar, constraints) in &self.constraints {
            let solved = self.solve_one(typevar, constraints)?;
            let _ = solutions.insert(typevar.clone(), solved);
        }

        // Apply defaults for `TypeVar`s with no constraints
        for (typevar, default) in &self.defaults {
            if !solutions.contains_key(typevar) {
                let _ = solutions.insert(typevar.clone(), default.clone());
            }
        }

        Ok(solutions)
    }

    /// Solve constraints for a single `TypeVar`.
    fn solve_one(
        &self,
        typevar: &str,
        constraints: &[TypeConstraint],
    ) -> Result<InferredType, SolveError> {
        let mut lower_bounds: Vec<InferredType> = Vec::new();
        let mut upper_bounds: Vec<InferredType> = Vec::new();
        let mut one_of_options: Option<&[InferredType]> = None;

        for constraint in constraints {
            match constraint {
                TypeConstraint::LowerBound(ty) => lower_bounds.push(ty.clone()),
                TypeConstraint::UpperBound(ty) => upper_bounds.push(ty.clone()),
                TypeConstraint::OneOf(options) => {
                    one_of_options = Some(options);
                }
            }
        }

        // 3d. Constrained `TypeVar` matching (§6.2)
        if let Some(options) = one_of_options {
            return Self::solve_constrained(typevar, &lower_bounds, options);
        }

        // 3b. Join lower bounds (union), meet upper bounds (intersection)
        let joined = if lower_bounds.is_empty() {
            None
        } else {
            Some(
                lower_bounds
                    .into_iter()
                    .reduce(InferredType::union)
                    .unwrap_or(InferredType::Never),
            )
        };

        // 3e. Bound `TypeVar` upper-bound check (§6.3)
        if let Some(ref solved) = joined {
            for bound in &upper_bounds {
                if !solved.is_assignable_to(bound) {
                    return Err(SolveError {
                        typevar: typevar.to_owned(),
                        reason: format!("solved type `{solved}` does not satisfy bound `{bound}`"),
                    });
                }
            }
        }

        // Use joined lower bounds, or upper bound, or default, or Unknown
        if let Some(solved) = joined {
            return Ok(solved);
        }

        if let Some(bound) = upper_bounds.into_iter().next() {
            return Ok(bound);
        }

        // 3f. Fall back to default
        if let Some(default) = self.defaults.get(typevar) {
            return Ok(default.clone());
        }

        Ok(InferredType::Unknown)
    }

    /// Solve a constrained `TypeVar` (`TypeVar("T", str, bytes)`).
    ///
    /// The solved type must be exactly one of the constraint options.
    /// If an argument is a subtype of a constraint, widen to the constraint.
    fn solve_constrained(
        typevar: &str,
        lower_bounds: &[InferredType],
        options: &[InferredType],
    ) -> Result<InferredType, SolveError> {
        if lower_bounds.is_empty() {
            // No arguments — use first option as default
            return options.first().cloned().ok_or_else(|| SolveError {
                typevar: typevar.to_owned(),
                reason: "constrained `TypeVar` has no options".to_owned(),
            });
        }

        // Find the option that all lower bounds are assignable to
        for option in options {
            if lower_bounds.iter().all(|lb| lb.is_assignable_to(option)) {
                return Ok(option.clone());
            }
        }

        Err(SolveError {
            typevar: typevar.to_owned(),
            reason: format!(
                "argument types do not match any constraint: expected one of [{}]",
                options
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
    }

    /// Check if this solver has any constraints.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty() && self.defaults.is_empty()
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}
