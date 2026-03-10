//! Basilisk compiler — compiles typed Python to native code.
//!
//! This crate takes Python source, runs it through the analyzer pipeline,
//! then JIT-compiles and executes it via Cranelift.
//!
//! The compilation pipeline:
//! ```text
//! .py → parse → resolve → check (GATE) → Cranelift JIT → native execution
//! ```

pub mod codegen;
pub mod error;
pub mod hir;

pub use error::CompileError;

use basilisk_checker::diagnostic::Diagnostic;

/// Result of compiling a single Python source file.
#[derive(Debug)]
pub struct CompileResult {
    /// Diagnostics from the type checker (must be empty to proceed to codegen).
    pub diagnostics: Vec<Diagnostic>,
    /// Stdout captured from native execution.
    pub stdout: String,
}

/// Compile and execute a Python source file.
///
/// This is the entry point for `basilisk run`. It:
/// 1. Parses the source
/// 2. Resolves names
/// 3. Runs the type checker
/// 4. If no errors, JIT-compiles via Cranelift and executes natively
///
/// # Errors
///
/// Returns [`CompileError`] if parsing, resolution, or code generation fails.
pub fn compile_and_run(source: &str, path: &str) -> Result<CompileResult, CompileError> {
    // Stage 1: Parse
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .map_err(|err| CompileError::Parse(err.to_string()))?;

    // Stage 2: Resolve
    let resolved = basilisk_resolver::resolve(&parsed)
        .map_err(|err| CompileError::Resolve(err.to_string()))?;

    // Stage 3: Type check (the gate)
    let diagnostics = basilisk_checker::check(&resolved);
    let errors: Vec<Diagnostic> = diagnostics
        .into_iter()
        .filter(|d| d.severity == basilisk_checker::diagnostic::Severity::Error)
        .collect();

    if !errors.is_empty() {
        return Ok(CompileResult {
            diagnostics: errors,
            stdout: String::new(),
        });
    }

    // Stage 4: JIT compile and execute natively (using resolved type info)
    let output = codegen::jit_compile_and_run(&parsed.ast, &resolved)?;

    Ok(CompileResult {
        diagnostics: Vec::new(),
        stdout: output,
    })
}
