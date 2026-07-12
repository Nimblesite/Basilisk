use basilisk_resolver::Span;

use crate::diagnostic::RuleMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Selector {
    Blanket,
    Specific(Vec<String>),
}

impl Selector {
    pub(super) fn matches(&self, code: &str) -> bool {
        match self {
            Self::Blanket => true,
            Self::Specific(codes) => codes.iter().any(|candidate| candidate == code),
        }
    }

    pub(super) fn is_blanket(&self) -> bool {
        matches!(self, Self::Blanket)
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Blanket, _) | (_, Self::Blanket) => true,
            (Self::Specific(left), Self::Specific(right)) => {
                left.iter().any(|code| right.contains(code))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Line(usize),
    Block { start: usize, end: Option<usize> },
    File,
}

impl Scope {
    pub(super) fn contains(self, line: usize) -> bool {
        match self {
            Self::Line(target) => line == target,
            Self::Block {
                start,
                end: Some(end),
            } => (start..=end).contains(&line),
            Self::Block { end: None, .. } => false,
            Self::File => true,
        }
    }

    pub(super) fn priority(self) -> u8 {
        match self {
            Self::Line(_) => 3,
            Self::Block { .. } => 2,
            Self::File => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Boundary {
    Ordinary,
    BlockStart,
    BlockEnd,
}

#[derive(Debug, Clone)]
pub(super) struct Directive {
    pub(super) span: Span,
    pub(super) scope: Scope,
    pub(super) boundary: Boundary,
    pub(super) mode: Option<RuleMode>,
    pub(super) selector: Selector,
    pub(super) problem: Option<String>,
    pub(super) paired_with: Option<usize>,
    pub(super) changed_diagnostics: usize,
}

impl Directive {
    pub(super) fn is_valid(&self) -> bool {
        self.problem.is_none() && self.mode.is_some()
    }

    pub(super) fn controls_diagnostics(&self) -> bool {
        self.is_valid() && self.boundary != Boundary::BlockEnd
    }
}
