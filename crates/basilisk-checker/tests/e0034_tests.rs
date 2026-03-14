#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0034: @final decorator violations.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0034_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0034")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0034_inherit_from_final_class_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import final

@final
class Base:
    pass

class Child(Base):
    pass
";
    let diags = run(source)?;
    let msgs = e0034_messages(&diags);
    assert!(
        msgs.iter()
            .any(|m| m.contains("Cannot inherit from final class")),
        "inheriting from @final class should fire E0034, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0034_final_on_module_function_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import final

@final
def my_func() -> None:
    pass
";
    let diags = run(source)?;
    let msgs = e0034_messages(&diags);
    assert!(
        msgs.iter().any(|m| m.contains("not allowed on non-method")),
        "@final on module-level function should fire E0034, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0034_override_final_method_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import final

class Base:
    @final
    def method(self) -> None:
        pass

class Child(Base):
    def method(self) -> None:
        pass
";
    let diags = run(source)?;
    let msgs = e0034_messages(&diags);
    assert!(
        msgs.iter()
            .any(|m| m.contains("overrides a `@final` method")),
        "overriding @final method should fire E0034, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0034_final_method_not_overridden_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import final

class Base:
    @final
    def method(self) -> None:
        pass

class Child(Base):
    def other_method(self) -> None:
        pass
";
    let diags = run(source)?;
    let msgs = e0034_messages(&diags);
    assert!(
        msgs.is_empty(),
        "not overriding @final method should not fire E0034, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0034_non_final_class_can_be_subclassed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    pass

class Child(Base):
    pass
";
    let diags = run(source)?;
    let msgs = e0034_messages(&diags);
    assert!(
        msgs.is_empty(),
        "subclassing non-final class should not fire E0034"
    );
    Ok(())
}
