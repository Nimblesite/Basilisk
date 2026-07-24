//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Integration tests for basilisk-lsp.

#[test]
fn lsp_returns_diagnostics_for_unannotated_function() {
    // The require-annotation house rules (BSK-0001/0002) are off by default —
    // the default config is pure PEP conformance — so opt in via config, exactly
    // as a project would. See [CHKARCH-CONFIGURATION-ONLY].
    let source = "def foo(x):\n    pass\n";
    let config = basilisk_config::BasiliskConfig::with_rule_entries(
        ["BSK-0001", "BSK-0002"]
            .into_iter()
            .map(|code| (code.to_owned(), basilisk_config::RuleSeverity::Error))
            .collect(),
    );
    let diags = basilisk_lsp::check_source_with_config(source, &config);
    assert!(
        !diags.is_empty(),
        "LSP must return diagnostics for unannotated function once house rules are enabled"
    );
}

#[test]
fn lsp_returns_no_diagnostics_for_clean_code() {
    let source = "def foo(x: int) -> int:\n    return x\n";
    let diags = basilisk_lsp::check_source(source);
    assert!(
        diags.is_empty(),
        "fully annotated code must produce no LSP diagnostics"
    );
}
