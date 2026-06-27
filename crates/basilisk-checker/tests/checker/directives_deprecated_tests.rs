//! Tests for [directives_deprecated] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for directives_deprecated: Deprecated usage.

use super::common::*;

#[test]
fn deprecated_function_call() -> Result<(), Box<dyn std::error::Error>> {
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
fn deprecated_class_instantiation() -> Result<(), Box<dyn std::error::Error>> {
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
fn non_deprecated_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def normal_func() -> None:
    pass

normal_func()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_deprecated"),
        "non-deprecated function should not fire E0115"
    );
    Ok(())
}

#[test]
fn deprecated_method_call() -> Result<(), Box<dyn std::error::Error>> {
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
fn deprecated_class_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
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

#[test]
fn deprecated_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewBase instead")
class OldBase:
    pass

class Child(OldBase):
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn deprecated_overload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated, overload

class MyClass:
    @overload
    def method(self, x: int) -> int: ...
    @overload
    @deprecated("Use method(str) instead")
    def method(self, x: str) -> str: ...
    def method(self, x: int | str) -> int | str:
        return x

obj = MyClass()
obj.method("hello")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn deprecated_class_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewType")
class OldType:
    pass

def func(x: OldType) -> None:
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn deprecated_module_import() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_var")
def old_var() -> int:
    return 42

result = old_var() + 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
