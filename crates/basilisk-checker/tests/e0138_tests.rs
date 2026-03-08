//! Integration tests for BSK-E0138: dataclass_transform metaclass violations.
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
fn e0138_transform_metaclass_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type): ...

class Model(metaclass=ModelMeta):
    id: int

class FrozenModel(Model, frozen=True):
    name: str

fm = FrozenModel(id=1, name="x")
fm.id = 2
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0138_transform_metaclass_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(kw_only_default=True)
class ModelMeta(type): ...

class Model(metaclass=ModelMeta):
    id: int
    name: str

m = Model(1, "x")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0138_valid_transform_metaclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type): ...

class Model(metaclass=ModelMeta):
    id: int

m = Model(id=1)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0138"),
        "valid transform metaclass should not fire E0138"
    );
    Ok(())
}
