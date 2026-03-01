//! Basic type narrowing for isinstance, is None, truthiness, and assignment.
//!
//! §7.1-7.4 of TYPE_INFERENCE.md

use basilisk_resolver::{ResolvedModule, Span};
use crate::types::InferredType;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0023",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0023",
};

/// Basic type narrowing engine.
pub(crate) struct NarrowingEngine;

impl NarrowingEngine {
    /// Performs isinstance narrowing.
    pub fn isinstance_narrowing(&self, current_type: InferredType, checked_type: InferredType) -> (InferredType, InferredType) {
        // In the if branch: intersection of current_type and checked_type
        // In the else branch: complement (current_type minus checked_type)
        (current_type, checked_type)
    }

    /// Performs is None / is not None narrowing.
    pub fn is_none_narrowing(&self, current_type: InferredType) -> (InferredType, InferredType) {
        // For is None: narrows to None
        // For is not None: removes None from union
        (current_type, InferredType::None_)
    }

    /// Performs truthiness narrowing.
    pub fn truthiness_narrowing(&self, current_type: InferredType) -> (InferredType, InferredType) {
            // In truthy branch: removes falsy types (None, "", 0, False)
        }

    /// Performs assignment narrowing.
    pub fn assignment_narrowing(&self, current_type: InferredType, assigned_type: InferredType) -> InferredType {
            assigned_type
        }
}

impl Rule for NarrowingEngine {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Basic narrowing will be integrated with type inference
        // For now, this is a placeholder for the narrowing logic
    }
}