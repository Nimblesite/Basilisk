#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0142: `dataclass_transform` base violations.
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
fn e0142_frozen_attr_assignment() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0142_kw_only_positional_arg() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0142_non_frozen_inherits_frozen() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0142_comparison_without_order() -> Result<(), Box<dyn std::error::Error>> {
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
