//! Implements [COMPARCH]. See docs/specs/COMPILER-ARCHITECTURE-SPEC.md#COMPARCH
//! Basilisk compiler — early prototype of the typed-Python execution path.
//!
//! Implements the analysis-and-gate front of [COMPILER-PIPELINE]: this crate
//! takes Python source and runs it through the analyzer pipeline, then executes
//! it. The spec's eventual back end (HIR → LLVM IR via inkwell → native code) is
//! NOT implemented; today the gate is followed by a tree-walking interpreter
//! (see `codegen.rs`), so execution semantics — not native codegen — are what
//! these stages currently deliver. See `codegen::jit_compile_and_run`.
//!
//! The compilation pipeline as implemented today:
//! ```text
//! .py → parse → resolve → check (GATE) → tree-walking interpreter → captured stdout
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
/// Implements [COMPILER-PIPELINE] — the parse → resolve → check (hard GATE)
/// stage sequence. Native compiler commands remain [COMPILERPLAN-NATIVE-INTEROP]
/// work; this is the library entry
/// point the e2e tests drive directly. It:
/// 1. Parses the source
/// 2. Resolves names
/// 3. Runs the type checker as the gate — any `Error` short-circuits before execution
/// 4. If no errors, executes via the tree-walking interpreter (NOT native codegen)
///
/// # Errors
///
/// Returns [`CompileError`] if parsing, resolution, or code generation fails.
pub fn compile_and_run(source: &str, path: &str) -> Result<CompileResult, CompileError> {
    // Stage 1: Parse ([COMPILER-PIPELINE])
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .map_err(|err| CompileError::Parse(err.to_string()))?;

    // Stage 2: Resolve
    let mut resolved = basilisk_resolver::resolve(&parsed)
        .map_err(|err| CompileError::Resolve(err.to_string()))?;

    // Attach the bundled Typeshed snapshot so stdlib imports (`typing`, `os`, …)
    // resolve in-process. Since [STUBRES-CUSTOM-TYPESHED] the checker recognises
    // stdlib names only through an attached snapshot — without one, `check`
    // reports every stdlib import as `imports_unresolved`.
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()
        .map_err(|err| CompileError::Resolve(err.to_string()))?;
    let search_paths = basilisk_checker::imports::ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(basilisk_checker::imports::ActiveTypeshed::new(
            std::sync::Arc::new(snapshot),
            None,
        )),
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &search_paths);

    // Stage 3: Type check — the hard GATE of [COMPILER-PIPELINE]
    // ("any Error stops compilation"). Code with errors never reaches execution.
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

    // Stage 4: Execute. Spec [COMPILER-CODEGEN] calls for HIR → LLVM IR → native
    // code; the current implementation is a tree-walking interpreter instead.
    let output = codegen::jit_compile_and_run(&parsed.ast, &resolved)?;

    Ok(CompileResult {
        diagnostics: Vec::new(),
        stdout: output,
    })
}
