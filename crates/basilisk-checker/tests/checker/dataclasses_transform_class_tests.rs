//! Tests for [dataclasses_transform_class] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for dataclasses_transform_class: `dataclass_transform` base violations.

use super::common::*;

#[test]
fn frozen_attr_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform
@dataclass_transform(kw_only_default=True)
class ModelBase: ...

class Customer(ModelBase, frozen=True):
    id: int

c = Customer(id=3)
c.id = 4
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn kw_only_positional_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform
@dataclass_transform(kw_only_default=True)
class ModelBase: ...

class Customer(ModelBase):
    id: int

c = Customer(3)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn non_frozen_inherits_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform
@dataclass_transform()
class ModelBase: ...

class Frozen(ModelBase, frozen=True):
    id: int

class NonFrozen(Frozen):
    name: str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn comparison_without_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform
@dataclass_transform()
class ModelBase: ...

class Item(ModelBase):
    value: int

a = Item(value=1)
b = Item(value=2)
result = a < b
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
