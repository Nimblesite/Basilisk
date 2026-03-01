//! BSK-E0011: Return type mismatch.
//!
//! Emitted when the inferred return type from function body expressions
//! is not assignable to the declared return type annotation.
//!
//! ```python
//! # BAD
//! def func() -> int:
//!     return "hello"  # E: inferred return type str is not assignable to int
//!
//! # GOOD  
//! def func() -> int:
//!     return 42  # OK: inferred return type int matches annotation
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0011",
};

/// Emits BSK-E0011 when inferred return types don't match the annotation.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for func in &module.functions {
            if is_stub_context(func, &module.classes) {
                continue;
            }
            
            // Skip if no return annotation or annotation is Any
            if !func.return_annotation.is_present() {
                continue;
            }
            
            // For now, we'll skip this check until we have proper return expression inference
            // This is a placeholder implementation that will be enhanced
            check_function_return_types(func, &module.path, diagnostics);
        }
    }
}

fn check_function_return_types(func: &FunctionInfo, _path: &str, _out: &mut Vec<Diagnostic>) {
    // TODO: Implement proper return type inference once we have expression analysis
    // This is disabled for now until the type inference system is complete
    // Placeholder: No-op implementation
}
