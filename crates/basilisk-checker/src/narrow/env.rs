//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! The scoped narrowing environment: branch push, complement, join, and
//! nested-function boundaries ([NARROWPLAN-CHECKLIST] Stage 2, flow analysis).

use std::collections::HashMap;

use crate::types::InferredType;

/// One branch frame's narrowed bindings, layered over the frames below it.
type Frame = HashMap<String, InferredType>;

/// A flow-scoped narrowing environment for one function body.
///
/// Layers of [`Frame`]s model control flow: entering a branch pushes a frame,
/// leaving pops it; a lookup walks frames innermost-first and falls back to
/// the declared (base) types. Narrowing NEVER changes the declared type used
/// for assignment validation — [`NarrowEnv::declared`] always answers from
/// the base layer ([NARROWPLAN-CHECKLIST]: "assignment narrowing without
/// changing the declared type").
///
/// Nested functions get a **fresh** environment ([`NarrowEnv::nested`]):
/// narrowing facts from the enclosing body do not flow in (a closure may run
/// after the narrow was invalidated), matching the boundary rule the spec and
/// every reference checker apply.
#[derive(Debug, Clone, Default)]
pub struct NarrowEnv {
    declared: Frame,
    /// Whole-scope narrowing facts (`assert`, post-early-exit complements,
    /// assignment narrowing outside any branch) — layered over `declared`
    /// WITHOUT mutating it, so the declared anchor survives.
    scope: Frame,
    frames: Vec<Frame>,
}

impl NarrowEnv {
    /// An environment seeded with the declared parameter/local types.
    #[must_use]
    pub fn new(declared: HashMap<String, InferredType>) -> Self {
        Self {
            declared,
            scope: Frame::new(),
            frames: Vec::new(),
        }
    }

    /// A fresh environment for a nested function: declared types only —
    /// no narrowing crosses the function boundary.
    #[must_use]
    pub fn nested(&self, declared: HashMap<String, InferredType>) -> Self {
        let _ = self;
        Self::new(declared)
    }

    /// The narrowed type currently visible for `name`, innermost frame first,
    /// then whole-scope facts, falling back to the declared type.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<InferredType> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.get(name))
            .or_else(|| self.scope.get(name))
            .or_else(|| self.declared.get(name))
            .cloned()
    }

    /// The DECLARED type for `name` — narrowing never touches this; it is
    /// what assignment compatibility keeps validating against.
    #[must_use]
    pub fn declared(&self, name: &str) -> Option<&InferredType> {
        self.declared.get(name)
    }

    /// Enter a branch: subsequent narrows apply only inside it.
    pub fn push_branch(&mut self) {
        self.frames.push(Frame::new());
    }

    /// Leave a branch, returning its narrowed bindings for a later
    /// [`NarrowEnv::join`] at the control-flow merge.
    pub fn pop_branch(&mut self) -> HashMap<String, InferredType> {
        self.frames.pop().unwrap_or_default()
    }

    /// Record a narrowing fact in the innermost open branch (or, outside any
    /// branch, as a whole-scope fact — the `assert` case). The declared
    /// layer is NEVER written: assignment validation keeps its anchor.
    pub fn narrow(&mut self, name: &str, ty: InferredType) {
        let frame = match self.frames.last_mut() {
            Some(frame) => frame,
            None => &mut self.scope,
        };
        let _ = frame.insert(name.to_owned(), ty);
    }

    /// Join two branch outcomes at a control-flow merge (`phi`): a name
    /// narrowed in BOTH branches flows on as the union of the two; a name
    /// narrowed in only one branch falls back to its declared type (the other
    /// path never narrowed it), so nothing over-narrows past the merge.
    pub fn join(
        &mut self,
        then_branch: HashMap<String, InferredType>,
        else_branch: HashMap<String, InferredType>,
    ) {
        let mut else_branch = else_branch;
        for (name, then_ty) in then_branch {
            if let Some(else_ty) = else_branch.remove(&name) {
                self.narrow(&name, InferredType::union(then_ty, else_ty));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrow::set_ops::{intersect, subtract};

    fn optional_int_env() -> NarrowEnv {
        NarrowEnv::new(
            [(
                "x".to_owned(),
                InferredType::Optional(Box::new(InferredType::Int)),
            )]
            .into_iter()
            .collect(),
        )
    }

    /// Branch narrowing is visible inside the branch and gone after popping.
    #[test]
    fn branch_narrowing_is_scoped() {
        let mut env = optional_int_env();
        env.push_branch();
        let narrowed = subtract(
            &env.lookup("x").unwrap_or(InferredType::Unknown),
            &InferredType::None_,
        );
        env.narrow("x", narrowed);
        assert_eq!(env.lookup("x"), Some(InferredType::Int));
        let _ = env.pop_branch();
        assert_eq!(
            env.lookup("x"),
            Some(InferredType::Optional(Box::new(InferredType::Int)))
        );
    }

    /// The declared type is never mutated by branch narrowing —
    /// assignment validation keeps its anchor.
    #[test]
    fn declared_type_survives_narrowing() {
        let mut env = optional_int_env();
        env.push_branch();
        env.narrow("x", InferredType::Int);
        assert_eq!(
            env.declared("x"),
            Some(&InferredType::Optional(Box::new(InferredType::Int)))
        );
    }

    /// A join unions both branches' narrowed types; one-sided narrows drop.
    #[test]
    fn join_unions_both_branches() {
        let mut env = NarrowEnv::new(
            [(
                "x".to_owned(),
                InferredType::Union(vec![
                    InferredType::Int,
                    InferredType::Str,
                    InferredType::None_,
                ]),
            )]
            .into_iter()
            .collect(),
        );

        env.push_branch();
        env.narrow("x", InferredType::Int);
        let then_branch = env.pop_branch();

        env.push_branch();
        env.narrow("x", InferredType::Str);
        let else_branch = env.pop_branch();

        env.join(then_branch, else_branch);
        let joined = env.lookup("x").unwrap_or(InferredType::Unknown);
        assert!(InferredType::Int.is_assignable_to(&joined));
        assert!(InferredType::Str.is_assignable_to(&joined));
        assert!(
            !InferredType::None_.is_assignable_to(&joined),
            "both branches eliminated None, so the merge keeps it eliminated"
        );
    }

    /// A name narrowed in only one branch reverts at the merge.
    #[test]
    fn one_sided_narrowing_reverts_at_merge() {
        let mut env = optional_int_env();
        env.push_branch();
        env.narrow("x", InferredType::Int);
        let then_branch = env.pop_branch();
        env.join(then_branch, HashMap::new());
        assert_eq!(
            env.lookup("x"),
            Some(InferredType::Optional(Box::new(InferredType::Int)))
        );
    }

    /// Nested functions start from declared types only.
    #[test]
    fn nested_function_boundary_drops_narrows() {
        let mut env = optional_int_env();
        env.narrow("x", InferredType::Int);
        let inner = env.nested(
            [(
                "x".to_owned(),
                InferredType::Optional(Box::new(InferredType::Int)),
            )]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            inner.lookup("x"),
            Some(InferredType::Optional(Box::new(InferredType::Int)))
        );
    }

    /// Whole-scope narrowing (assert) applies without an open branch and
    /// composes with intersect.
    #[test]
    fn assert_narrows_the_whole_scope() {
        let mut env = optional_int_env();
        let narrowed = intersect(
            &env.lookup("x").unwrap_or(InferredType::Unknown),
            &InferredType::Int,
        );
        env.narrow("x", narrowed);
        assert_eq!(env.lookup("x"), Some(InferredType::Int));
    }
}
