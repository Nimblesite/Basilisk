//! Type narrowing context that consumes resolver-collected `NarrowingGuard`s.
//!
//! The resolver walks the AST and collects `NarrowingGuard` values for each
//! function body (isinstance, is-None, truthiness, assignment, assert,
//! `TypeGuard`, `TypeIs`, match).  This module builds a scope-aware narrowing
//! context that rules can query to determine the narrowed type of a variable
//! at a given source position.

use basilisk_resolver::{NarrowingGuard, NarrowingGuardKind, Span};

use crate::types::InferredType;

/// A narrowing context for a single function body.
///
/// Built from the resolver's `NarrowingGuard` list, it provides
/// `narrowed_type_at()` to query the type of a variable at a given
/// byte offset in the source.
#[derive(Debug)]
pub struct NarrowingContext {
    /// Narrowing entries sorted by span start for binary search.
    entries: Vec<NarrowingEntry>,
}

/// A single narrowing entry: at a given source range, a variable
/// has a specific narrowed type.
#[derive(Debug, Clone)]
struct NarrowingEntry {
    /// The variable being narrowed.
    variable: String,
    /// The narrowed type within this scope.
    narrowed_type: InferredType,
    /// The source span where this narrowing is active.
    active_span: Span,
    /// Whether this narrowing is inside a loop (doesn't persist after loop).
    _in_loop: bool,
}

impl NarrowingContext {
    /// Build a narrowing context from a list of resolver-collected guards.
    #[must_use]
    pub fn from_guards(guards: &[NarrowingGuard]) -> Self {
        let mut entries = Vec::new();

        for guard in guards {
            Self::process_guard(&guard.kind, guard.in_loop, &mut entries);
        }

        // Sort by active_span.start for efficient lookup.
        entries.sort_by_key(|entry| entry.active_span.start);

        Self { entries }
    }

    /// Process a single guard kind into narrowing entries.
    fn process_guard(kind: &NarrowingGuardKind, in_loop: bool, entries: &mut Vec<NarrowingEntry>) {
        match kind {
            NarrowingGuardKind::IsInstance {
                variable,
                type_names,
                if_body_span,
                else_body_span,
            } => Self::process_isinstance(
                variable,
                type_names,
                *if_body_span,
                else_body_span.as_ref(),
                in_loop,
                entries,
            ),
            NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                if_body_span,
                else_body_span,
            } => Self::process_is_none(
                variable,
                *is_positive,
                *if_body_span,
                else_body_span.as_ref(),
                in_loop,
                entries,
            ),
            NarrowingGuardKind::Truthiness {
                variable,
                if_body_span,
                else_body_span,
            } => Self::process_truthiness(
                variable,
                *if_body_span,
                else_body_span.as_ref(),
                in_loop,
                entries,
            ),
            NarrowingGuardKind::Assignment {
                variable,
                assigned_type,
            } => Self::process_assignment(variable, assigned_type.as_ref(), in_loop, entries),
            NarrowingGuardKind::Assert { inner } => {
                Self::process_assert_guard(inner, in_loop, entries);
            }
            NarrowingGuardKind::TypeGuard {
                variable,
                guard_type,
                if_body_span,
                ..
            } => Self::process_type_guard(variable, guard_type, *if_body_span, in_loop, entries),
            NarrowingGuardKind::TypeIs {
                variable,
                guard_type,
                if_body_span,
                else_body_span,
            } => Self::process_type_is(
                variable,
                guard_type,
                *if_body_span,
                else_body_span.as_ref(),
                in_loop,
                entries,
            ),
            NarrowingGuardKind::Match {
                variable, cases, ..
            } => Self::process_match(variable, cases, in_loop, entries),
        }
    }

    /// Process an `isinstance` guard into positive and negative narrowing entries.
    fn process_isinstance(
        variable: &str,
        type_names: &[String],
        if_body_span: Span,
        else_body_span: Option<&Span>,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        // Positive branch: variable is narrowed to the union of type_names.
        let narrowed = types_from_names(type_names);
        entries.push(NarrowingEntry {
            variable: variable.to_owned(),
            narrowed_type: narrowed,
            active_span: if_body_span,
            _in_loop: in_loop,
        });

        // Negative branch: variable has complement type (we mark as Unknown
        // since computing the complement requires knowing the original type).
        if let Some(else_span) = else_body_span {
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: InferredType::Unknown,
                active_span: *else_span,
                _in_loop: in_loop,
            });
        }
    }

    /// Process an `is None` / `is not None` guard.
    fn process_is_none(
        variable: &str,
        is_positive: bool,
        if_body_span: Span,
        else_body_span: Option<&Span>,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        // `is None` positive: narrowed to None.
        // `is not None`: narrowed to non-None (Unknown without original type).
        let if_type = if is_positive {
            InferredType::None_
        } else {
            InferredType::Unknown // Removing None requires original type
        };
        entries.push(NarrowingEntry {
            variable: variable.to_owned(),
            narrowed_type: if_type,
            active_span: if_body_span,
            _in_loop: in_loop,
        });

        if let Some(else_span) = else_body_span {
            let else_type = if is_positive {
                InferredType::Unknown // Non-None complement
            } else {
                InferredType::None_
            };
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: else_type,
                active_span: *else_span,
                _in_loop: in_loop,
            });
        }
    }

    /// Process a truthiness guard (truthy/falsy branch).
    fn process_truthiness(
        variable: &str,
        if_body_span: Span,
        else_body_span: Option<&Span>,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        // Truthy branch: remove falsy values (None, False, 0, "", etc.)
        // We can't compute the exact narrowed type without the original.
        entries.push(NarrowingEntry {
            variable: variable.to_owned(),
            narrowed_type: InferredType::Unknown,
            active_span: if_body_span,
            _in_loop: in_loop,
        });

        if let Some(else_span) = else_body_span {
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: InferredType::Unknown,
                active_span: *else_span,
                _in_loop: in_loop,
            });
        }
    }

    /// Process an assignment guard.
    fn process_assignment(
        variable: &str,
        assigned_type: Option<&String>,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        // Assignment narrows to the assigned type.
        let narrowed = assigned_type
            .map(String::as_str)
            .map_or(InferredType::Unknown, InferredType::from_annotation);
        // Assignment narrowing applies from the assignment point forward.
        // We don't have a precise span, so we skip (handled by flow analysis).
        if !matches!(narrowed, InferredType::Unknown) {
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: narrowed,
                active_span: Span {
                    start: 0,
                    end: u32::MAX,
                },
                _in_loop: in_loop,
            });
        }
    }

    /// Process a `TypeGuard` — narrows only in the positive branch (per spec section 7.6).
    fn process_type_guard(
        variable: &str,
        guard_type: &str,
        if_body_span: Span,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        let narrowed = InferredType::from_annotation(guard_type);
        entries.push(NarrowingEntry {
            variable: variable.to_owned(),
            narrowed_type: narrowed,
            active_span: if_body_span,
            _in_loop: in_loop,
        });
    }

    /// Process a `TypeIs` guard — narrows bidirectionally.
    fn process_type_is(
        variable: &str,
        guard_type: &str,
        if_body_span: Span,
        else_body_span: Option<&Span>,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        let narrowed = InferredType::from_annotation(guard_type);
        entries.push(NarrowingEntry {
            variable: variable.to_owned(),
            narrowed_type: narrowed.clone(),
            active_span: if_body_span,
            _in_loop: in_loop,
        });

        // Negative branch: complement (Unknown without original type).
        if let Some(else_span) = else_body_span {
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: InferredType::Unknown,
                active_span: *else_span,
                _in_loop: in_loop,
            });
        }
    }

    /// Process a match statement — each case narrows the subject to the pattern type.
    fn process_match(
        variable: &str,
        cases: &[basilisk_resolver::MatchCaseNarrowing],
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        for case in cases {
            let narrowed = InferredType::from_annotation(&case.pattern_type);
            entries.push(NarrowingEntry {
                variable: variable.to_owned(),
                narrowed_type: narrowed,
                active_span: case.body_span,
                _in_loop: in_loop,
            });
        }
    }

    /// Process an assert guard — narrows unconditionally for subsequent code.
    fn process_assert_guard(
        kind: &NarrowingGuardKind,
        in_loop: bool,
        entries: &mut Vec<NarrowingEntry>,
    ) {
        match kind {
            NarrowingGuardKind::IsInstance {
                variable,
                type_names,
                ..
            } => {
                let narrowed = types_from_names(type_names);
                entries.push(NarrowingEntry {
                    variable: variable.clone(),
                    narrowed_type: narrowed,
                    active_span: Span {
                        start: 0,
                        end: u32::MAX,
                    },
                    _in_loop: in_loop,
                });
            }
            NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                ..
            } => {
                let narrowed = if *is_positive {
                    InferredType::None_
                } else {
                    InferredType::Unknown
                };
                entries.push(NarrowingEntry {
                    variable: variable.clone(),
                    narrowed_type: narrowed,
                    active_span: Span {
                        start: 0,
                        end: u32::MAX,
                    },
                    _in_loop: in_loop,
                });
            }
            _ => {} // Other assert forms are uncommon.
        }
    }

    /// Query the narrowed type of a variable at a given byte offset.
    ///
    /// Returns `Some(narrowed_type)` if the variable has been narrowed at
    /// this position, `None` if no narrowing applies.
    #[must_use]
    pub fn narrowed_type_at(&self, variable: &str, offset: u32) -> Option<&InferredType> {
        // Find the most specific (smallest span) narrowing entry that
        // contains this offset and matches the variable name.
        self.entries
            .iter()
            .filter(|entry| {
                entry.variable == variable
                    && entry.active_span.start <= offset
                    && offset < entry.active_span.end
            })
            .min_by_key(|entry| entry.active_span.end - entry.active_span.start)
            .map(|entry| &entry.narrowed_type)
    }

    /// Returns `true` if there are any narrowing entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Convert a list of type names to an `InferredType`.
fn types_from_names(names: &[String]) -> InferredType {
    match names.len() {
        0 => InferredType::Unknown,
        1 => names.first().map_or(InferredType::Unknown, |name| {
            InferredType::from_annotation(name)
        }),
        _ => {
            let types: Vec<InferredType> = names
                .iter()
                .map(|name| InferredType::from_annotation(name))
                .collect();
            InferredType::Union(types)
        }
    }
}
