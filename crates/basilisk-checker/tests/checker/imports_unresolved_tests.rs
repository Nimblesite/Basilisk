//! Tests for [`imports_unresolved`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Also exercises the static resolution model [STUBRES-STATIC-MODEL]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-STATIC-MODEL
// Integration tests for imports_unresolved: Import from untyped module.

use super::common::*;

#[test]
fn stdlib_import_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import os\nimport sys\nimport typing\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "stdlib imports should not fire E0010"
    );
    Ok(())
}

#[test]
fn typing_import_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from typing import List, Dict\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "typing imports should not fire E0010"
    );
    Ok(())
}

#[test]
fn third_party_import_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import numpy\n")?;
    assert!(
        codes(&diags).contains(&"imports_unresolved"),
        "third-party import should fire E0010"
    );
    Ok(())
}

#[test]
fn third_party_from_import_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from pandas import DataFrame\n")?;
    assert!(
        codes(&diags).contains(&"imports_unresolved"),
        "third-party from-import should fire E0010"
    );
    Ok(())
}

#[test]
fn submodule_stdlib_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from os.path import join\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "stdlib submodule import should not fire E0010"
    );
    Ok(())
}

#[test]
fn collections_abc_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from collections.abc import Mapping\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "collections.abc import should not fire E0010"
    );
    Ok(())
}

#[test]
fn typing_extensions_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from typing_extensions import TypeAlias\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "typing_extensions import should not fire E0010"
    );
    Ok(())
}

#[test]
fn dataclasses_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from dataclasses import dataclass\n")?;
    assert!(
        !codes(&diags).contains(&"imports_unresolved"),
        "dataclasses import should not fire E0010"
    );
    Ok(())
}

#[test]
fn diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import requests\n")?;
    let e0010 = diags.iter().find(|d| d.code.code == "imports_unresolved");
    assert!(e0010.is_some(), "should fire E0010 for requests");
    let Some(diag) = e0010 else {
        return Err("E0010 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0010 should have help text");
    Ok(())
}

/// [STUBRES-STATIC-MODEL]: resolution is a static filesystem search, so a module
/// that only a runtime `sys.meta_path` hook could ever provide is invisible to
/// Basilisk — it matches no resolution step and surfaces as `imports_unresolved`
/// exactly like any other unresolved import. The checker never executes the
/// program to honour the hook.
#[test]
fn meta_path_provided_module_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import sys\n",
        "class _Finder: ...\n",
        "sys.meta_path.insert(0, _Finder())\n",
        "import magic_runtime_module\n",
    );
    let diags = run(src)?;
    assert!(
        codes(&diags).contains(&"imports_unresolved"),
        "a module only a runtime meta_path hook could supply must surface as unresolved"
    );
    Ok(())
}
