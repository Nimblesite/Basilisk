//! Basilisk compiler — compiles typed Python to native code.
//!
//! This crate takes a type-checked [`basilisk_resolver::ResolvedModule`],
//! lowers it to a high-level IR, and generates LLVM IR for native execution.
//!
//! The compilation pipeline:
//! ```text
//! .py → parse → resolve → check (GATE) → HIR → LLVM IR → machine code
//! ```

pub mod error;
pub mod hir;

pub use error::CompileError;

use basilisk_checker::diagnostic::Diagnostic;

/// Result of compiling a single Python source file.
///
/// For now this is a stub — the real implementation will produce LLVM IR.
/// The bare-bones version just runs the analyzer pipeline and captures output.
#[derive(Debug)]
pub struct CompileResult {
    /// Diagnostics from the type checker (must be empty to proceed to codegen).
    pub diagnostics: Vec<Diagnostic>,
    /// Stdout captured from execution (only populated in interpret mode).
    pub stdout: String,
}

/// Compile and execute a Python source file.
///
/// This is the entry point for `basilisk run`. Currently it:
/// 1. Parses the source
/// 2. Resolves names
/// 3. Runs the type checker
/// 4. If no errors, executes via Python interpreter (temporary until LLVM codegen lands)
///
/// # Errors
///
/// Returns [`CompileError`] if parsing, resolution, or execution fails.
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

    // Stage 4: Execute (temporary — runs via Python interpreter until LLVM codegen lands)
    let output = execute_via_python(source)?;

    Ok(CompileResult {
        diagnostics: Vec::new(),
        stdout: output,
    })
}

/// Temporary execution backend — runs source through the system Python interpreter.
///
/// This will be replaced by LLVM JIT execution once codegen is implemented.
fn execute_via_python(source: &str) -> Result<String, CompileError> {
    use std::process::Command;

    let mut child = Command::new("python3")
        .arg("-c")
        .arg(source)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| CompileError::Execution(format!("failed to spawn python3: {err}")))?;

    // Drop stdin so the child doesn't hang
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|err| CompileError::Execution(format!("failed to wait on python3: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompileError::Execution(format!(
            "python3 exited with {}: {stderr}",
            output.status
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
