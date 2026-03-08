//! Integration tests for BSK-E0004: Missing *args/**kwargs type annotation.
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
fn e0004_unannotated_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(*args) -> None:\n    pass\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated *args should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0004_unannotated_kwargs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(**kwargs) -> None:\n    pass\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated **kwargs should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0004_annotated_args_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(*args: int) -> None:\n    pass\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated *args should not fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_annotated_kwargs_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(**kwargs: str) -> None:\n    pass\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated **kwargs should not fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_both_unannotated_fires_twice() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(*args, **kwargs) -> None:\n    pass\n")?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0004").count();
    assert_eq!(count, 2, "both unannotated *args and **kwargs should fire E0004");
    Ok(())
}

#[test]
fn e0004_stub_body_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def foo(*args) -> None:\n    ...\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "stub body should be exempt from E0004"
    );
    Ok(())
}
