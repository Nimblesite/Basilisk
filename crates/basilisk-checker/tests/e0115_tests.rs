//! Integration tests for BSK-E0115: Deprecated usage.
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
fn e0115_deprecated_function_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None: ...

old_func()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_deprecated_class_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewClass instead")
class OldClass:
    pass

x = OldClass()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_non_deprecated_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def normal_func() -> None:
    pass

normal_func()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0115"),
        "non-deprecated function should not fire E0115"
    );
    Ok(())
}

#[test]
fn e0115_deprecated_method_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class MyClass:
    @deprecated("Use new_method instead")
    def old_method(self) -> None: ...

    def new_method(self) -> None: ...

obj = MyClass()
obj.old_method()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_deprecated_class_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use new_func")
def old_func() -> None: ...

x = old_func
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
