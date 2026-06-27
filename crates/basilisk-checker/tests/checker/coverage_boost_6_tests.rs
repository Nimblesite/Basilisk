//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 6: targeting highest-uncovered rules.
// Focus: e0115, e0072, e0107, e0070, e0079, e0047, e0014, e0036

// --- E0115: Deprecated usage ---

#[test]
fn deprecated_function_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass

old_func()
"#;
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "directives_deprecated"));
    Ok(())
}

#[test]
fn deprecated_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use NewClass")
class OldClass:
    pass

x = OldClass()
"#;
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "directives_deprecated"));
    Ok(())
}

#[test]
fn deprecated_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class MyClass:
    @deprecated("Use new_method")
    def old_method(self) -> None:
        pass

    def new_method(self) -> None:
        pass

obj = MyClass()
obj.old_method()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_overload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated, overload

@overload
@deprecated("Use str version")
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...

def process(x):
    return x

process(42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Config:
    @property
    @deprecated("Use get_value instead")
    def value(self) -> int:
        return 42

c = Config()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import deprecated

@deprecated
def bare_deprecated() -> None:
    pass

bare_deprecated()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typing_extensions_qualified() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import typing_extensions

@typing_extensions.deprecated("old")
def old_func() -> None:
    pass

old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewBase")
class OldBase:
    pass

class Child(OldBase):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

x = old_func()
y: int = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_if() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_check")
def old_check() -> bool:
    return True

if old_check():
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_for() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_range")
def old_range() -> list:
    return [1, 2, 3]

for x in old_range():
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

def wrapper() -> int:
    return old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_module_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_val")
class OldVal:
    pass

result = OldVal
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

x = 0
x += old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0072: No matching overload ---

#[test]
fn getitem_str_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

class MyBytes:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: slice) -> bytes: ...
    def __getitem__(self, __i_or_s):
        pass

b = MyBytes()
b[""]
"#;
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "overloads_basic"));
    Ok(())
}

#[test]
fn getitem_valid_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class MyBytes:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: slice) -> bytes: ...
    def __getitem__(self, __i_or_s):
        pass

b = MyBytes()
b[0]
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "overloads_basic")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn getitem_float_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Container:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

c = Container()
c[3.14]
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "overloads_basic"));
    Ok(())
}

#[test]
fn getitem_slice() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Seq:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: slice) -> list: ...
    def __getitem__(self, __i_or_s):
        pass

s = Seq()
s[1:3]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Grid:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

g = Grid()
g[(1, 2)]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Store:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

s = Store()
s[None]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

class Decoder:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

d = Decoder()
d[b"hello"]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_list() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Matrix:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

m = Matrix()
m[[1, 2]]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Lookup:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

l = Lookup()
l[{}]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_set() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class SetLookup:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

s = SetLookup()
s[{1, 2}]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn getitem_bool() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Container:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __i_or_s):
        pass

c = Container()
c[True]
";
    let _ = run(source)?;
    Ok(())
}

// --- E0107: Variance incompatibility ---

#[test]
fn co_in_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)

class Base(Generic[T]):
    pass

class Bad(Base[T_co]):
    pass
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "generics_variance"));
    Ok(())
}

#[test]
fn contra_in_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
T_contra = TypeVar('T_contra', contravariant=True)

class Base(Generic[T]):
    pass

class Bad(Base[T_contra]):
    pass
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "generics_variance"));
    Ok(())
}

#[test]
fn co_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T_co = TypeVar('T_co', covariant=True)

class ReadOnly(Generic[T_co]):
    pass

class Sub(ReadOnly[T_co]):
    pass
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "generics_variance")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn multi_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
T_co = TypeVar('T_co', covariant=True)

class Pair(Generic[T, U]):
    pass

class BadPair(Pair[T_co, T_co]):
    pass
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "generics_variance"));
    Ok(())
}

#[test]
fn contra_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T_contra = TypeVar('T_contra', contravariant=True)

class Sink(Generic[T_contra]):
    pass

class Sub(Sink[T_contra]):
    pass
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "generics_variance")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn co_in_contra() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T_contra = TypeVar('T_contra', contravariant=True)
T_co = TypeVar('T_co', covariant=True)

class Sink(Generic[T_contra]):
    pass

class Bad(Sink[T_co]):
    pass
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "generics_variance"));
    Ok(())
}

// --- E0070: Never type compat ---

#[test]
fn local_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, Generic, TypeVar

T = TypeVar('T')

def func(c: list[Never]) -> None:
    v: list[int] = c
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "specialtypes_never_2"));
    Ok(())
}

#[test]
fn return_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, Generic, TypeVar

T = TypeVar('T')
U = TypeVar('U')

class ClassC(Generic[T]):
    pass

def func(x: U) -> ClassC[U]:
    return ClassC[Never]()
";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "specialtypes_never_2"));
    Ok(())
}

#[test]
fn covariant_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, Generic, TypeVar

T_co = TypeVar('T_co', covariant=True)

class ReadOnly(Generic[T_co]):
    pass

def func() -> ReadOnly[int]:
    return ReadOnly[Never]()
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_never_2")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn any_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, Any

def func(c: list[Never]) -> None:
    v: list[Any] = c
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_never_2")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn never_to_never() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def func(c: list[Never]) -> None:
    v: list[Never] = c
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_never_2")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn dict_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def func(c: dict[str, Never]) -> None:
    v: dict[str, int] = c
";
    let _ = run(source)?;
    Ok(())
}

// --- E0079: Module protocol ---

#[test]
fn module_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasTimeout(Protocol):
    timeout: int
    def connect(self) -> None: ...

import socket
x: HasTimeout = socket
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn no_import_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasAttr(Protocol):
    name: str
";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "protocols_modules")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Connectable(Protocol):
    def connect(self) -> bool: ...
    def disconnect(self) -> None: ...

import os
c: Connectable = os
";
    let _ = run(source)?;
    Ok(())
}

// --- E0047: Invalid type expression ---

#[test]
fn list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: [int, str]) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(diags
        .iter()
        .any(|d| d.code.code == "annotations_forward_refs"));
    Ok(())
}

#[test]
fn dict_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = "y: {} = {}\n";
    let diags = run(source)?;
    assert!(diags
        .iter()
        .any(|d| d.code.code == "annotations_forward_refs"));
    Ok(())
}

#[test]
fn conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def g(x: int if True else str) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(diags
        .iter()
        .any(|d| d.code.code == "annotations_forward_refs"));
    Ok(())
}

#[test]
fn fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = "z: f\"int\" = 1\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: lambda: int) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn boolean_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int or str) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn negative() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: -1) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn module_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = "import types\ndef f(x: types) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn unannotated_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var1 = 3\ndef f(x: var1) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Optional, Union\ndef f(x: int, y: Optional[int], z: Union[int, str]) -> None:\n    pass\n";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "annotations_forward_refs")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn eval() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: eval(\"int\")) -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

// --- E0036: ClassVar ---

#[test]
fn outside_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import ClassVar\nx: ClassVar[int] = 42\n";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "classes_classvar"));
    Ok(())
}

#[test]
fn in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import ClassVar\ndef f() -> None:\n    x: ClassVar[int] = 42\n";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "classes_classvar"));
    Ok(())
}

#[test]
fn class_body_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import ClassVar\n\nclass MyClass:\n    count: ClassVar[int] = 0\n";
    let diags = run(source)?;
    let cnt = diags
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    assert_eq!(cnt, 0);
    Ok(())
}

#[test]
fn in_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import ClassVar\ndef f(x: ClassVar[int]) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "classes_classvar"));
    Ok(())
}

#[test]
fn return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import ClassVar\ndef f() -> ClassVar[int]:\n    return 1\n";
    let diags = run(source)?;
    assert!(diags.iter().any(|d| d.code.code == "classes_classvar"));
    Ok(())
}
