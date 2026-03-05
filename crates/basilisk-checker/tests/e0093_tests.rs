//! Tests for BSK-E0093: Invalid key or value type in `TypedDict` assignment.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run_e2e(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn test_e0093_subscript_with_bad_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
movie["director"] = "Ridley Scott"  # Invalid key
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        !e0093.is_empty(),
        "Subscript with bad key should fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_subscript_correct_key_wrong_value_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
movie["year"] = "1982"  # Wrong type: str instead of int
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        !e0093.is_empty(),
        "Subscript with wrong value type should fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_dict_literal_with_unknown_keys() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"title": "Blade Runner", "year": 1982}  # Invalid key 'title'
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        !e0093.is_empty(),
        "Dict literal with unknown keys should fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_pop_called_with_required_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
movie.pop("name")  # Cannot pop required key
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(!e0093.is_empty(), "pop() on required key should fire E0093");
    Ok(())
}

#[test]
fn test_e0093_update_called_with_unknown_keys() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
movie.update({"director": "Ridley Scott"})  # Unknown key 'director'
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        !e0093.is_empty(),
        "update() with unknown keys should fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_delete_subscript_on_required_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
del movie["name"]  # Cannot delete required key
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(!e0093.is_empty(), "del on required key should fire E0093");
    Ok(())
}

#[test]
fn test_e0093_valid_typeddict_usage_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        e0093.is_empty(),
        "Valid TypedDict usage should not fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_subscript_read_invalid_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
director = movie["director"]  # Invalid key read
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        !e0093.is_empty(),
        "Subscript read with invalid key should fire E0093"
    );
    Ok(())
}

#[test]
fn test_e0093_non_literal_dict_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

key = "name"
movie: Movie = {key: "Blade Runner", "year": 1982}  # Non-literal key
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(!e0093.is_empty(), "Non-literal dict key should fire E0093");
    Ok(())
}

#[test]
fn test_e0093_total_false_missing_keys_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict, total=False):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner"}  # Missing 'year' is OK when total=False
"#;
    let diags = run_e2e(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0093")
        .collect();
    assert!(
        e0093.is_empty(),
        "Missing keys with total=False should not fire E0093"
    );
    Ok(())
}
