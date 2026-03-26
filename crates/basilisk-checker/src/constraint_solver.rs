//! `TypeVar` constraint solver for generic type inference.
//!
//! Collects type constraints from function call arguments and return
//! types, then solves for the concrete types of `TypeVar`s.  Supports
//! lower-bound join, upper-bound meet, constrained `TypeVar`s (PEP 484 §6.2),
//! bounded `TypeVar`s (§6.3), and `TypeVar` defaults (PEP 696 §6.5).

use std::collections::HashMap;

use crate::types::InferredType;

/// A single constraint on a `TypeVar`.
#[derive(Debug, Clone)]
enum Constraint {
    /// The `TypeVar` must be a supertype of this type (lower bound).
    LowerBound(InferredType),
    /// The `TypeVar` must be a subtype of this type (upper bound).
    UpperBound(InferredType),
    /// The `TypeVar` must be exactly one of these types (constrained).
    OneOf(Vec<InferredType>),
}

/// Solver for `TypeVar` constraints in a single generic call site.
#[derive(Debug)]
pub struct ConstraintSolver {
    /// `TypeVar` name -> list of constraints.
    constraints: HashMap<String, Vec<Constraint>>,
    /// `TypeVar` name -> default type (PEP 696).
    defaults: HashMap<String, InferredType>,
    /// `TypeVar` name -> upper bound type.
    bounds: HashMap<String, InferredType>,
}

impl ConstraintSolver {
    /// Create a new empty solver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            constraints: HashMap::new(),
            defaults: HashMap::new(),
            bounds: HashMap::new(),
        }
    }

    /// Add a lower-bound constraint: `TypeVar >= type`.
    pub fn add_lower_bound(&mut self, typevar: &str, bound: InferredType) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(Constraint::LowerBound(bound));
    }

    /// Add an upper-bound constraint: `TypeVar <= type`.
    pub fn add_upper_bound(&mut self, typevar: &str, bound: InferredType) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(Constraint::UpperBound(bound));
    }

    /// Add a "one of" constraint for constrained `TypeVar`s.
    pub fn add_one_of(&mut self, typevar: &str, options: Vec<InferredType>) {
        self.constraints
            .entry(typevar.to_owned())
            .or_default()
            .push(Constraint::OneOf(options));
    }

    /// Add a return type constraint from bidirectional inference.
    pub fn add_return_constraint(&mut self, typevar: &str, expected_return: InferredType) {
        self.add_upper_bound(typevar, expected_return);
    }

    /// Set the default type for a `TypeVar` (PEP 696).
    pub fn set_default(&mut self, typevar: &str, default: InferredType) {
        let _ = self.defaults.insert(typevar.to_owned(), default);
    }

    /// Set the bound for a `TypeVar` (PEP 484 §6.3).
    pub fn set_bound(&mut self, typevar: &str, bound: InferredType) {
        let _ = self.bounds.insert(typevar.to_owned(), bound);
    }

    /// Solve all constraints and return the resolved type for each `TypeVar`.
    #[must_use]
    pub fn solve(&self) -> HashMap<String, InferredType> {
        let mut solutions = HashMap::new();

        for (typevar, constraints) in &self.constraints {
            let solution = self.solve_one(typevar, constraints);
            let _ = solutions.insert(typevar.clone(), solution);
        }

        // Fill in defaults for unconstrained `TypeVar`s.
        for (typevar, default) in &self.defaults {
            let _: &mut _ = solutions
                .entry(typevar.clone())
                .or_insert_with(|| default.clone());
        }

        solutions
    }

    /// Solve constraints for a single `TypeVar`.
    fn solve_one(&self, typevar: &str, constraints: &[Constraint]) -> InferredType {
        let mut lower_bounds: Vec<&InferredType> = Vec::new();
        let mut upper_bounds: Vec<&InferredType> = Vec::new();
        let mut constrained_options: Option<&Vec<InferredType>> = None;

        for constraint in constraints {
            match constraint {
                Constraint::LowerBound(ty) => lower_bounds.push(ty),
                Constraint::UpperBound(ty) => upper_bounds.push(ty),
                Constraint::OneOf(options) => constrained_options = Some(options),
            }
        }

        // Constrained `TypeVar`: find the narrowest option that satisfies all lower bounds.
        if let Some(options) = constrained_options {
            return Self::solve_constrained(&lower_bounds, options);
        }

        // Join lower bounds: the solution must be a supertype of all lower bounds.
        let joined = if lower_bounds.is_empty() {
            // No lower bounds — check defaults.
            self.defaults
                .get(typevar)
                .cloned()
                .unwrap_or(InferredType::Unknown)
        } else if lower_bounds.len() == 1 {
            lower_bounds
                .first()
                .map_or(InferredType::Unknown, |b| (*b).clone())
        } else {
            // Create a union of all lower bounds.
            let first = lower_bounds
                .first()
                .map_or(InferredType::Unknown, |b| (*b).clone());
            lower_bounds
                .iter()
                .skip(1)
                .fold(first, |acc, &bound| InferredType::union(acc, bound.clone()))
        };

        // Validate against upper bounds.
        for upper in &upper_bounds {
            if !joined.is_assignable_to(upper) {
                // Upper bound violated — fall back to the upper bound.
                return (*upper).clone();
            }
        }

        // Validate against `TypeVar` bound.
        if let Some(bound) = self.bounds.get(typevar) {
            if !joined.is_assignable_to(bound) {
                return bound.clone();
            }
        }

        joined
    }

    /// Solve a constrained `TypeVar` by finding the narrowest option
    /// that satisfies all lower bounds.
    fn solve_constrained(lower_bounds: &[&InferredType], options: &[InferredType]) -> InferredType {
        // Find the first option that all lower bounds are assignable to.
        for option in options {
            let all_satisfy = lower_bounds
                .iter()
                .all(|bound| bound.is_assignable_to(option));
            if all_satisfy {
                return option.clone();
            }
        }

        // Widening: try float for int, complex for float, etc.
        for option in options {
            let all_satisfy_with_widening = lower_bounds
                .iter()
                .all(|bound| bound.is_assignable_to(option) || is_numeric_widening(bound, option));
            if all_satisfy_with_widening {
                return option.clone();
            }
        }

        // No option satisfies — return the first option as a best-effort.
        options.first().cloned().unwrap_or(InferredType::Unknown)
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if `source` can be widened to `target` via numeric coercion.
fn is_numeric_widening(source: &InferredType, target: &InferredType) -> bool {
    matches!(
        (source, target),
        (InferredType::Bool, InferredType::Int | InferredType::Float)
            | (InferredType::Int, InferredType::Float)
    )
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only: expect acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn solver_single_lower_bound() {
        let mut solver = ConstraintSolver::new();
        solver.add_lower_bound("T", InferredType::Int);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Int));
    }

    #[test]
    fn solver_multiple_lower_bounds_creates_union() {
        let mut solver = ConstraintSolver::new();
        solver.add_lower_bound("T", InferredType::Int);
        solver.add_lower_bound("T", InferredType::Str);
        let solutions = solver.solve();
        let solution = solutions.get("T").expect("should have T");
        assert!(matches!(solution, InferredType::Union(_)));
    }

    #[test]
    fn solver_upper_bound_fallback() {
        let mut solver = ConstraintSolver::new();
        solver.add_lower_bound("T", InferredType::Str);
        solver.add_upper_bound("T", InferredType::Int);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Int));
    }

    #[test]
    fn solver_default_for_unconstrained() {
        let mut solver = ConstraintSolver::new();
        solver.set_default("T", InferredType::Str);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Str));
    }

    #[test]
    fn solver_constrained_typevar() {
        let mut solver = ConstraintSolver::new();
        solver.add_one_of("T", vec![InferredType::Int, InferredType::Str]);
        solver.add_lower_bound("T", InferredType::Int);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Int));
    }

    #[test]
    fn solver_bound_validation() {
        let mut solver = ConstraintSolver::new();
        solver.set_bound("T", InferredType::Float);
        solver.add_lower_bound("T", InferredType::Str);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Float));
    }

    #[test]
    fn solver_return_constraint_acts_as_upper_bound() {
        let mut solver = ConstraintSolver::new();
        solver.add_lower_bound("T", InferredType::Str);
        solver.add_return_constraint("T", InferredType::Int);
        let solutions = solver.solve();
        assert_eq!(solutions.get("T"), Some(&InferredType::Int));
    }

    #[test]
    fn solver_empty_has_no_solutions() {
        let solver = ConstraintSolver::new();
        let solutions = solver.solve();
        assert!(solutions.is_empty());
    }

    #[test]
    fn numeric_widening_int_to_float() {
        assert!(is_numeric_widening(
            &InferredType::Int,
            &InferredType::Float
        ));
    }

    #[test]
    fn numeric_widening_bool_to_int() {
        assert!(is_numeric_widening(&InferredType::Bool, &InferredType::Int));
    }

    #[test]
    fn numeric_widening_str_to_int_is_false() {
        assert!(!is_numeric_widening(&InferredType::Str, &InferredType::Int));
    }

    #[test]
    fn default_impl_creates_empty_solver() {
        let solver = ConstraintSolver::default();
        assert!(solver.solve().is_empty());
    }
}
