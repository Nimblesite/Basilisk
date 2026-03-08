//! Integration tests for BSK-E0010: Import from untyped module.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn e0010_stdlib_import_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import os\nimport sys\nimport typing\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "stdlib imports should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_typing_import_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from typing import List, Dict\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "typing imports should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_third_party_import_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import numpy\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0010"),
        "third-party import should fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_third_party_from_import_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from pandas import DataFrame\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0010"),
        "third-party from-import should fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_submodule_stdlib_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from os.path import join\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "stdlib submodule import should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_collections_abc_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from collections.abc import Mapping\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "collections.abc import should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_typing_extensions_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from typing_extensions import TypeAlias\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "typing_extensions import should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_dataclasses_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("from dataclasses import dataclass\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0010"),
        "dataclasses import should not fire E0010"
    );
    Ok(())
}

#[test]
fn e0010_diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("import requests\n")?;
    let e0010 = diags.iter().find(|d| d.code.code == "BSK-E0010");
    assert!(e0010.is_some(), "should fire E0010 for requests");
    let Some(diag) = e0010 else {
        return Err("E0010 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0010 should have help text");
    Ok(())
}
