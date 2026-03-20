//! Type narrowing engine for control-flow-sensitive type analysis.
//!
//! Consumes [`NarrowingGuard`]s collected by the resolver and tracks
//! narrowed variable types through branches and join points.
//!
//! See `CHECKER-TYPE-INFERENCE-SPEC.md` §7.1–7.10.

use std::collections::HashMap;

use basilisk_resolver::{NarrowingGuard, NarrowingGuardKind};

use crate::types::InferredType;

/// Tracks narrowed variable types through control flow within a function.
///
/// The context maintains a stack of type states for branch/join points:
/// - At a branch (`if`/`else`, `match`), the current state is forked.
/// - At a join point, the two states are merged (union of types).
/// - Assert guards narrow for all subsequent code (no branch).
#[derive(Debug, Clone)]
pub struct NarrowingContext {
    /// Variable name → current narrowed type.
    type_state: HashMap<String, InferredType>,
    /// Stack of saved states for branch/join points.
    branch_stack: Vec<HashMap<String, InferredType>>,
}

impl NarrowingContext {
    /// Create a new narrowing context, optionally seeded with parameter types.
    #[must_use]
    pub fn new() -> Self {
        Self {
            type_state: HashMap::new(),
            branch_stack: Vec::new(),
        }
    }

    /// Seed the context with parameter types from function annotations.
    pub fn seed_parameters(&mut self, params: &[(String, String)]) {
        for (name, annotation) in params {
            let inferred = InferredType::from_annotation(annotation);
            let _ = self.type_state.insert(name.clone(), inferred);
        }
    }

    /// Query the current narrowed type of a variable.
    #[must_use]
    pub fn get_type(&self, var: &str) -> Option<&InferredType> {
        self.type_state.get(var)
    }

    /// Narrow a variable's type in the current scope.
    pub fn narrow(&mut self, var: &str, narrowed: InferredType) {
        let _ = self.type_state.insert(var.to_owned(), narrowed);
    }

    /// Save the current type state before entering a branch.
    pub fn push_branch(&mut self) {
        self.branch_stack.push(self.type_state.clone());
    }

    /// Restore the saved state and merge with the current branch state.
    ///
    /// After this call, each variable's type is the union of its type
    /// in both branches — reflecting that either branch could have executed.
    pub fn pop_and_join(&mut self) {
        let Some(saved) = self.branch_stack.pop() else {
            return;
        };

        let mut joined = HashMap::new();

        // Variables present in both branches: union their types
        for (name, current_type) in &self.type_state {
            if let Some(saved_type) = saved.get(name) {
                let union = InferredType::union(current_type.clone(), saved_type.clone());
                let _ = joined.insert(name.clone(), union);
            } else {
                let _ = joined.insert(name.clone(), current_type.clone());
            }
        }

        // Variables only in the saved branch
        for (name, saved_type) in &saved {
            if !self.type_state.contains_key(name) {
                let _ = joined.insert(name.clone(), saved_type.clone());
            }
        }

        self.type_state = joined;
    }

    /// Swap the current state with the saved branch state.
    ///
    /// Used to process the `else` branch: after processing the `if` body,
    /// swap to get the pre-if state, apply negative narrowing, then process
    /// the else body.
    pub fn swap_with_saved(&mut self) {
        if let Some(saved) = self.branch_stack.last_mut() {
            std::mem::swap(&mut self.type_state, saved);
        }
    }

    /// Process all narrowing guards for a function and return the final type state.
    ///
    /// This is the main entry point. It processes guards in order, applying
    /// narrowing to the type state as it goes.
    #[must_use]
    pub fn process_guards(mut self, guards: &[NarrowingGuard]) -> HashMap<String, InferredType> {
        for guard in guards {
            self.apply_guard(guard);
        }
        self.type_state
    }

    /// Apply a single narrowing guard to the current type state.
    fn apply_guard(&mut self, guard: &NarrowingGuard) {
        // Guards inside loops produce narrowing only within the loop body,
        // not after. We still record them so queries within the loop work,
        // but the join after the loop resets narrowing.
        match &guard.kind {
            NarrowingGuardKind::IsInstance {
                variable,
                type_names,
                ..
            } => self.apply_isinstance(variable, type_names),

            NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                ..
            } => self.apply_is_none(variable, *is_positive),

            NarrowingGuardKind::Truthiness { variable, .. } => {
                self.apply_truthiness(variable);
            }

            NarrowingGuardKind::Assignment {
                variable,
                assigned_type,
            } => self.apply_assignment(variable, assigned_type.as_deref()),

            NarrowingGuardKind::Assert { inner } => {
                // Assert narrows for ALL subsequent code — apply the positive
                // narrowing without branching.
                self.apply_assert_inner(inner);
            }

            NarrowingGuardKind::TypeGuard {
                variable,
                guard_type,
                ..
            } => self.apply_typeguard(variable, guard_type),

            NarrowingGuardKind::TypeIs {
                variable,
                guard_type,
                ..
            } => self.apply_typeis(variable, guard_type),

            NarrowingGuardKind::Match {
                variable, cases, ..
            } => self.apply_match(variable, cases),
        }
    }

    /// §7.1: isinstance narrowing.
    ///
    /// In the positive branch, `x` is narrowed to the intersection of its
    /// current type and the checked type(s). For now, we set it to the
    /// checked type directly (full intersection requires subtyping engine).
    fn apply_isinstance(&mut self, variable: &str, type_names: &[String]) {
        let narrowed = if let [single] = type_names {
            InferredType::from_annotation(single)
        } else {
            // isinstance(x, (A, B)) narrows to A | B in the positive branch
            let types: Vec<InferredType> = type_names
                .iter()
                .map(|n| InferredType::from_annotation(n))
                .collect();
            types
                .into_iter()
                .reduce(InferredType::union)
                .unwrap_or(InferredType::Unknown)
        };

        let _ = self.type_state.insert(variable.to_owned(), narrowed);
    }

    /// §7.2: None narrowing.
    ///
    /// `x is None` narrows to `None` in the positive branch.
    /// `x is not None` removes `None` from the type in the positive branch.
    fn apply_is_none(&mut self, variable: &str, is_positive: bool) {
        if is_positive {
            // `x is None` → x: None
            let _ = self
                .type_state
                .insert(variable.to_owned(), InferredType::None_);
        } else {
            // `x is not None` → remove None from type
            if let Some(current) = self.type_state.get(variable) {
                let narrowed = remove_none(current);
                let _ = self.type_state.insert(variable.to_owned(), narrowed);
            }
        }
    }

    /// §7.3: Truthiness narrowing.
    ///
    /// `if x:` removes falsy types (`None`, `Literal[0]`, `Literal[""]`,
    /// `Literal[False]`) in the truthy branch.
    fn apply_truthiness(&mut self, variable: &str) {
        if let Some(current) = self.type_state.get(variable) {
            let narrowed = remove_falsy(current);
            let _ = self.type_state.insert(variable.to_owned(), narrowed);
        }
    }

    /// §7.4: Assignment narrowing.
    ///
    /// After `x = expr`, the type of `x` is the type of `expr`.
    fn apply_assignment(&mut self, variable: &str, assigned_type: Option<&str>) {
        if let Some(type_text) = assigned_type {
            let narrowed = InferredType::from_annotation(type_text);
            let _ = self.type_state.insert(variable.to_owned(), narrowed);
        }
    }

    /// §7.8: Assert narrowing — apply the inner guard unconditionally.
    fn apply_assert_inner(&mut self, inner: &NarrowingGuardKind) {
        match inner {
            NarrowingGuardKind::IsInstance {
                variable,
                type_names,
                ..
            } => self.apply_isinstance(variable, type_names),

            NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                ..
            } => {
                // `assert x is not None` → remove None (is_positive=false for "is not None")
                self.apply_is_none(variable, *is_positive);
            }

            NarrowingGuardKind::Truthiness { variable, .. } => {
                self.apply_truthiness(variable);
            }

            _ => {} // Other guard kinds not typical in assert
        }
    }

    /// §7.6: `TypeGuard` narrowing — positive branch only.
    ///
    /// The variable is narrowed to the guard type in the positive branch.
    /// The negative branch retains the original type (no narrowing).
    fn apply_typeguard(&mut self, variable: &str, guard_type: &str) {
        let narrowed = InferredType::from_annotation(guard_type);
        let _ = self.type_state.insert(variable.to_owned(), narrowed);
    }

    /// §7.7: `TypeIs` narrowing — bidirectional.
    ///
    /// Positive branch: narrowed to the guard type.
    /// Negative branch: complement (original minus guard type).
    /// For now, we only apply positive narrowing (complement requires
    /// full subtyping engine).
    fn apply_typeis(&mut self, variable: &str, guard_type: &str) {
        let narrowed = InferredType::from_annotation(guard_type);
        let _ = self.type_state.insert(variable.to_owned(), narrowed);
    }

    /// §7.5: Match narrowing.
    ///
    /// Each case narrows the subject to the pattern type.
    fn apply_match(&mut self, variable: &str, cases: &[basilisk_resolver::MatchCaseNarrowing]) {
        // For the match subject, the type after the match is the union of all
        // case pattern types. Individual case body narrowing is handled by
        // queries within the case body span.
        if cases.is_empty() {
            return;
        }

        let types: Vec<InferredType> = cases
            .iter()
            .map(|c| InferredType::from_annotation(&c.pattern_type))
            .collect();

        let union = types
            .into_iter()
            .reduce(InferredType::union)
            .unwrap_or(InferredType::Unknown);

        let _ = self.type_state.insert(variable.to_owned(), union);
    }

    /// Query the narrowed type of a variable at a specific byte offset.
    ///
    /// Searches through the guards to find which narrowing applies at
    /// the given offset. Returns `None` if no narrowing applies.
    #[must_use]
    pub fn type_at_offset(
        &self,
        variable: &str,
        offset: u32,
        guards: &[NarrowingGuard],
    ) -> Option<InferredType> {
        // Walk guards in reverse to find the most recent applicable narrowing
        for guard in guards.iter().rev() {
            if let Some(narrowed) = self.guard_type_at_offset(variable, offset, guard) {
                return Some(narrowed);
            }
        }
        self.type_state.get(variable).cloned()
    }

    /// Check if a guard narrows a variable at a specific offset.
    fn guard_type_at_offset(
        &self,
        variable: &str,
        offset: u32,
        guard: &NarrowingGuard,
    ) -> Option<InferredType> {
        match &guard.kind {
            NarrowingGuardKind::IsInstance {
                variable: guard_var,
                type_names,
                if_body_span,
                else_body_span,
            } if guard_var == variable => {
                if if_body_span.contains_offset(offset) {
                    // Positive branch: narrowed to checked type
                    return Some(isinstance_positive_type(type_names));
                }
                if let Some(else_span) = else_body_span {
                    if else_span.contains_offset(offset) {
                        // Negative branch: complement (original - checked)
                        return self
                            .type_state
                            .get(variable)
                            .map(|t| remove_types(t, type_names));
                    }
                }
                None
            }

            NarrowingGuardKind::IsNone {
                variable: guard_var,
                is_positive,
                if_body_span,
                else_body_span,
            } if guard_var == variable => self.is_none_at_offset(
                variable,
                offset,
                *is_positive,
                *if_body_span,
                else_body_span.as_ref(),
            ),

            NarrowingGuardKind::Truthiness {
                variable: guard_var,
                if_body_span,
                else_body_span,
            } if guard_var == variable => {
                if if_body_span.contains_offset(offset) {
                    return self.type_state.get(variable).map(remove_falsy);
                }
                if let Some(else_span) = else_body_span {
                    if else_span.contains_offset(offset) {
                        // Falsy branch — only None / Literal[False] etc. remain
                        return self.type_state.get(variable).map(keep_falsy);
                    }
                }
                None
            }

            NarrowingGuardKind::Assert { inner } => {
                // Assert narrows for ALL code after the assert statement
                if guard.span.end <= offset {
                    return self.assert_type_for(variable, inner);
                }
                None
            }

            NarrowingGuardKind::TypeGuard {
                variable: guard_var,
                guard_type,
                if_body_span,
                ..
            } if guard_var == variable => {
                if if_body_span.contains_offset(offset) {
                    return Some(InferredType::from_annotation(guard_type));
                }
                // Negative branch: NOT narrowed for TypeGuard (§7.6)
                None
            }

            NarrowingGuardKind::TypeIs {
                variable: guard_var,
                guard_type,
                if_body_span,
                else_body_span,
            } if guard_var == variable => {
                if if_body_span.contains_offset(offset) {
                    return Some(InferredType::from_annotation(guard_type));
                }
                if let Some(else_span) = else_body_span {
                    if else_span.contains_offset(offset) {
                        // Negative branch: complement
                        return self
                            .type_state
                            .get(variable)
                            .map(|t| remove_types(t, &[guard_type.clone()]));
                    }
                }
                None
            }

            NarrowingGuardKind::Match {
                variable: guard_var,
                cases,
                ..
            } if guard_var == variable => {
                for case in cases {
                    if case.body_span.contains_offset(offset) {
                        return Some(InferredType::from_annotation(&case.pattern_type));
                    }
                }
                None
            }

            _ => None,
        }
    }

    /// Resolve `is None` / `is not None` narrowing at a specific offset.
    fn is_none_at_offset(
        &self,
        variable: &str,
        offset: u32,
        is_positive: bool,
        if_body_span: basilisk_resolver::Span,
        else_body_span: Option<&basilisk_resolver::Span>,
    ) -> Option<InferredType> {
        if if_body_span.contains_offset(offset) {
            return Some(if is_positive {
                InferredType::None_
            } else {
                self.type_state
                    .get(variable)
                    .map_or(InferredType::Unknown, remove_none)
            });
        }
        if let Some(else_span) = else_body_span {
            if else_span.contains_offset(offset) {
                return Some(if is_positive {
                    // else of `is None` → not None
                    self.type_state
                        .get(variable)
                        .map_or(InferredType::Unknown, remove_none)
                } else {
                    // else of `is not None` → is None
                    InferredType::None_
                });
            }
        }
        None
    }

    /// Get the narrowed type for a variable from an assert guard's inner kind.
    fn assert_type_for(&self, variable: &str, inner: &NarrowingGuardKind) -> Option<InferredType> {
        match inner {
            NarrowingGuardKind::IsInstance {
                variable: guard_var,
                type_names,
                ..
            } if guard_var == variable => Some(isinstance_positive_type(type_names)),

            NarrowingGuardKind::IsNone {
                variable: guard_var,
                is_positive,
                ..
            } if guard_var == variable => Some(if *is_positive {
                InferredType::None_
            } else {
                self.type_state
                    .get(variable)
                    .map_or(InferredType::Unknown, remove_none)
            }),

            NarrowingGuardKind::Truthiness {
                variable: guard_var,
                ..
            } if guard_var == variable => self.type_state.get(variable).map(remove_falsy),

            _ => None,
        }
    }
}

impl Default for NarrowingContext {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Build the positive-branch type for isinstance narrowing.
fn isinstance_positive_type(type_names: &[String]) -> InferredType {
    if let [single] = type_names {
        InferredType::from_annotation(single)
    } else {
        type_names
            .iter()
            .map(|n| InferredType::from_annotation(n))
            .reduce(InferredType::union)
            .unwrap_or(InferredType::Unknown)
    }
}

/// Remove `None` from a type (for `is not None` narrowing).
fn remove_none(ty: &InferredType) -> InferredType {
    match ty {
        InferredType::None_ => InferredType::Never,
        InferredType::Optional(inner) => inner.as_ref().clone(),
        InferredType::Union(types) => {
            let filtered: Vec<InferredType> = types
                .iter()
                .filter(|t| !matches!(t, InferredType::None_))
                .cloned()
                .collect();
            match filtered.len() {
                0 => InferredType::Never,
                1 => filtered.into_iter().next().unwrap_or(InferredType::Never),
                _ => InferredType::Union(filtered),
            }
        }
        other => other.clone(),
    }
}

/// Remove falsy types from a union (for truthiness narrowing).
///
/// Falsy types: `None`, `Literal[0]`, `Literal[""]`, `Literal[False]`.
fn remove_falsy(ty: &InferredType) -> InferredType {
    match ty {
        InferredType::None_
        | InferredType::Literal(
            crate::types::LiteralValue::Bool(false) | crate::types::LiteralValue::Int(0),
        ) => InferredType::Never,
        InferredType::Literal(crate::types::LiteralValue::Str(s)) if s.is_empty() => {
            InferredType::Never
        }
        InferredType::Optional(inner) => inner.as_ref().clone(),
        InferredType::Union(types) => {
            let filtered: Vec<InferredType> = types
                .iter()
                .filter(|t| !is_falsy_type(t))
                .cloned()
                .collect();
            match filtered.len() {
                0 => InferredType::Never,
                1 => filtered.into_iter().next().unwrap_or(InferredType::Never),
                _ => InferredType::Union(filtered),
            }
        }
        other => other.clone(),
    }
}

/// Keep only falsy types (for else branch of truthiness narrowing).
fn keep_falsy(ty: &InferredType) -> InferredType {
    match ty {
        InferredType::Optional(_) | InferredType::None_ => InferredType::None_,
        InferredType::Union(types) => {
            let falsy: Vec<InferredType> =
                types.iter().filter(|t| is_falsy_type(t)).cloned().collect();
            match falsy.len() {
                0 => InferredType::Never,
                1 => falsy.into_iter().next().unwrap_or(InferredType::Never),
                _ => InferredType::Union(falsy),
            }
        }
        other if is_falsy_type(other) => other.clone(),
        _ => InferredType::Never,
    }
}

/// Check if a type is falsy (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`).
fn is_falsy_type(ty: &InferredType) -> bool {
    matches!(
        ty,
        InferredType::None_
            | InferredType::Literal(
                crate::types::LiteralValue::Bool(false) | crate::types::LiteralValue::Int(0),
            )
    ) || matches!(ty, InferredType::Literal(crate::types::LiteralValue::Str(ref s)) if s.is_empty())
}

/// Remove specific named types from a union (for isinstance complement).
fn remove_types(ty: &InferredType, type_names: &[String]) -> InferredType {
    match ty {
        InferredType::Union(types) => {
            let filtered: Vec<InferredType> = types
                .iter()
                .filter(|t| {
                    let type_str = t.to_string();
                    !type_names.contains(&type_str)
                })
                .cloned()
                .collect();
            match filtered.len() {
                0 => InferredType::Never,
                1 => filtered.into_iter().next().unwrap_or(InferredType::Never),
                _ => InferredType::Union(filtered),
            }
        }
        other => {
            let type_str = other.to_string();
            if type_names.contains(&type_str) {
                InferredType::Never
            } else {
                other.clone()
            }
        }
    }
}

/// Build a `NarrowingContext` for a function, seeded with parameter types.
#[must_use]
pub fn build_context_for_function(func: &basilisk_resolver::FunctionInfo) -> NarrowingContext {
    let mut ctx = NarrowingContext::new();

    let params: Vec<(String, String)> = func
        .parameters
        .iter()
        .filter_map(|p| {
            let ann = p.annotation_text.as_ref()?;
            Some((p.name.clone(), ann.clone()))
        })
        .collect();

    ctx.seed_parameters(&params);
    ctx
}
