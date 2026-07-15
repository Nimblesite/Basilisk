use basilisk_resolver::Span;

use crate::diagnostic::RuleMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Selector {
    Blanket,
    Specific(Vec<String>),
}

impl Selector {
    pub(crate) fn is_blanket(&self) -> bool {
        matches!(self, Self::Blanket)
    }

    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Blanket, _) | (_, Self::Blanket) => true,
            (Self::Specific(left), Self::Specific(right)) => selectors_overlap(left, right),
        }
    }
}

pub(crate) fn selectors_overlap(left: &[String], right: &[String]) -> bool {
    left.is_empty() || right.is_empty() || left.iter().any(|code| right.contains(code))
}

pub(crate) fn mark_conflict_pair(conflicts: &mut [bool], left: usize, right: usize) {
    for index in [left, right] {
        if let Some(conflict) = conflicts.get_mut(index) {
            *conflict = true;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Line(usize),
    Block { start: usize, end: Option<usize> },
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Boundary {
    Ordinary,
    BlockStart,
    BlockEnd,
}

#[derive(Debug, Clone)]
pub(crate) struct Directive {
    pub(crate) span: Span,
    pub(crate) scope: Scope,
    pub(crate) boundary: Boundary,
    pub(crate) mode: Option<RuleMode>,
    pub(crate) selector: Selector,
    pub(crate) problem: Option<String>,
    pub(crate) paired_with: Option<usize>,
    pub(crate) changed_diagnostics: usize,
}

impl Directive {
    pub(crate) fn is_valid(&self) -> bool {
        self.problem.is_none() && self.mode.is_some()
    }
}

pub(crate) fn mark_conflicts(directives: &mut [Directive]) {
    let mut conflicts = vec![false; directives.len()];
    for left_index in 0..directives.len() {
        let Some(left) = directives.get(left_index) else {
            continue;
        };
        for (right_index, right) in directives.iter().enumerate().skip(left_index + 1) {
            if !left.is_valid() || !right.is_valid() {
                continue;
            }
            let same_scope = match (left.scope, right.scope) {
                (Scope::Line(a), Scope::Line(b)) => a == b,
                (Scope::File, Scope::File) => true,
                _ => false,
            };
            if same_scope && left.mode != right.mode && left.selector.overlaps(&right.selector) {
                mark_conflict_pair(&mut conflicts, left_index, right_index);
            }
        }
    }
    for (directive, conflict) in directives.iter_mut().zip(conflicts) {
        if conflict {
            directive.problem = Some("conflicting directives at the same scope".to_owned());
        }
    }
}
