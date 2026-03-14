//! Tests for resolver: test_final_violations.

mod common;

use common::resolve_src;

#[test]
fn class_info_from_qualified_dataclass_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Point:\n",
        "    x: int = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Point")
        .ok_or("Point not found")?;
    assert!(
        cls.is_dataclass,
        "qualified @dataclasses.dataclass must set is_dataclass"
    );
    Ok(())
}

#[test]
fn class_info_from_bare_dataclass_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class Rect:\n",
        "    w: int = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Rect")
        .ok_or("Rect not found")?;
    assert!(cls.is_dataclass, "bare @dataclass must set is_dataclass");
    Ok(())
}

#[test]
fn class_info_from_qualified_final_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        "@typing.final\n",
        "class Sealed:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Sealed")
        .ok_or("Sealed not found")?;
    assert!(cls.is_final, "qualified @typing.final must set is_final");
    Ok(())
}

#[test]
fn final_class_attr_without_init_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "Final class attr without init must produce a violation"
    );
    Ok(())
}

#[test]
fn final_class_attr_with_value_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 42\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.is_empty(),
        "Final class attr with value must not produce a violation"
    );
    Ok(())
}

#[test]
fn final_instance_reassignment_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 42\n",
        "    def __init__(self) -> None:\n",
        "        self.x = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "reassigning Final attr in __init__ when class-level value exists must violate"
    );
    Ok(())
}

#[test]
fn final_instance_outside_init_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    def method(self) -> None:\n",
        "        self.x: Final[int] = 42\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "Final annotation outside __init__ must produce a violation"
    );
    Ok(())
}

#[test]
fn final_subclass_override_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Parent:\n",
        "    x: Final[int] = 1\n",
        "class Child(Parent):\n",
        "    x: int = 2\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "overriding Final attr in subclass must produce a violation"
    );
    Ok(())
}

#[test]
fn final_function_local_modification() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "def foo() -> None:\n",
        "    x: Final[int] = 42\n",
        "    x = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "modifying a function-local Final must produce a violation"
    );
    Ok(())
}

#[test]
fn final_instance_modify_in_method() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 42\n",
        "    def change(self) -> None:\n",
        "        self.x = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "modifying Final attr in non-init method must produce a violation"
    );
    Ok(())
}

#[test]
fn final_walrus_operator_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "def foo() -> None:\n",
        "    x: Final[int] = 42\n",
        "    y = (x := 99)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "walrus reassignment of Final must produce a violation"
    );
    Ok(())
}

#[test]
fn final_augmented_assignment_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "def foo() -> None:\n",
        "    x: Final[int] = 42\n",
        "    x += 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "augmented assignment to Final must produce a violation"
    );
    Ok(())
}

#[test]
fn final_global_modification_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "X: Final[int] = 42\n",
        "def modify() -> None:\n",
        "    global X\n",
        "    X = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "modifying global Final var must produce a violation"
    );
    Ok(())
}

#[test]
fn final_for_loop_target_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "def foo() -> None:\n",
        "    x: Final[int] = 42\n",
        "    for x in [1, 2, 3]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "for loop target reassigning Final must produce a violation"
    );
    Ok(())
}

#[test]
fn final_with_target_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "def foo() -> None:\n",
        "    x: Final[int] = 42\n",
        "    with open('f') as x:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "with target reassigning Final must produce a violation"
    );
    Ok(())
}

#[test]
fn class_final_without_init_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.iter().any(|v| v.name == "x"),
        "Final attr without init or value should be a violation"
    );
    Ok(())
}

#[test]
fn subclass_override_final_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Base:\n",
        "    x: Final[int] = 42\n",
        "class Child(Base):\n",
        "    x: int = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.iter().any(|v| v.name == "x"),
        "overriding a Final attr in a subclass should be a violation"
    );
    Ok(())
}

#[test]
fn instance_final_outside_init_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 10\n",
        "    def mutate(self) -> None:\n",
        "        self.x: Final[int] = 20\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.final_violations.is_empty(),
        "self.x: Final outside __init__ should be a violation"
    );
    Ok(())
}

#[test]
fn instance_modify_final_via_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 10\n",
        "    def change(self) -> None:\n",
        "        self.x = 20\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.iter().any(|v| v.name == "x"),
        "self.x = ... in non-__init__ should be InstanceModifyFinal"
    );
    Ok(())
}

#[test]
fn instance_modify_final_via_aug_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 10\n",
        "    def bump(self) -> None:\n",
        "        self.x += 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.iter().any(|v| v.name == "x"),
        "self.x += ... should be InstanceModifyFinal"
    );
    Ok(())
}

#[test]
fn instance_reassign_already_initialized() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int] = 10\n",
        "    def __init__(self) -> None:\n",
        "        self.x = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.iter().any(|v| v.name == "x"),
        "self.x = ... in __init__ when class already has value should be a violation"
    );
    Ok(())
}

#[test]
fn class_is_final_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import final\n",
        "@final\n",
        "class Sealed:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Sealed");
    assert!(cls.is_some_and(|c| c.is_final));
    Ok(())
}

#[test]
fn class_defined_inside_try_finally() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "try:\n",
        "    class TryClass:\n",
        "        def m(self) -> None: ...\n",
        "except:\n",
        "    pass\n",
        "finally:\n",
        "    class FinallyClass:\n",
        "        def m(self) -> None: ...\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "TryClass"));
    assert!(resolved.classes.iter().any(|c| c.name == "FinallyClass"));
    Ok(())
}
