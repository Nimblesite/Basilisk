//! Integration tests for basilisk-lsp.

#[test]
fn lsp_returns_diagnostics_for_unannotated_function() {
    // Phase 2: the LSP must report diagnostics for code with type errors.
    // Currently returns an empty list (placeholder).
    let source = "def foo(x):\n    pass\n";
    let diags = basilisk_lsp::check_source(source);
    assert!(
        !diags.is_empty(),
        "LSP must return diagnostics for unannotated function — Phase 2 not yet implemented"
    );
}

#[test]
fn lsp_returns_no_diagnostics_for_clean_code() {
    // Phase 2: fully annotated code must produce no diagnostics once the LSP
    // is implemented.  Currently returns empty regardless (placeholder).
    let source = "def foo(x: int) -> int:\n    return x\n";
    let diags = basilisk_lsp::check_source(source);
    assert!(
        diags.is_empty(),
        "fully annotated code must produce no LSP diagnostics"
    );
}
