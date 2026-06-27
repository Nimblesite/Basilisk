//! Tests for [typeddicts_operations] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Tests for typeddicts_operations: Invalid key or value type in `TypedDict` assignment.

use super::common::*;

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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
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
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
        .collect();
    assert!(
        e0093.is_empty(),
        "Missing keys with total=False should not fire E0093"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// TypedDict assignment compatibility (exercises type_consistency.rs)
// ---------------------------------------------------------------------------

#[test]
fn test_e0093_typeddict_to_dict_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
d: dict = movie
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_to_mapping_object() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict, Mapping

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
m: Mapping[str, object] = movie
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_to_mapping_narrow_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict, Mapping

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
m: Mapping[str, str] = movie
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_structural_compat_matching() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class MovieA(TypedDict):
    name: str
    year: int

class MovieB(TypedDict):
    name: str
    year: int

a: MovieA = {"name": "Blade Runner", "year": 1982}
b: MovieB = a
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_structural_compat_missing_field() -> Result<(), Box<dyn std::error::Error>>
{
    let src = r#"
from typing import TypedDict

class Small(TypedDict):
    name: str

class Big(TypedDict):
    name: str
    year: int
    genre: str

small: Small = {"name": "Blade Runner"}
big: Big = small
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_structural_compat_type_mismatch() -> Result<(), Box<dyn std::error::Error>>
{
    let src = r#"
from typing import TypedDict

class MovieA(TypedDict):
    name: str
    year: int

class MovieB(TypedDict):
    name: str
    year: str

a: MovieA = {"name": "Blade Runner", "year": 1982}
b: MovieB = a
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_to_dict_with_params() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}
d: dict[str, int] = movie
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

class Film(TypedDict):
    name: str
    year: int
    director: str

movie: Movie = {"name": "Blade Runner", "year": 1982}
film: Film = {"name": "Blade Runner", "year": 1982, "director": "Scott"}
movie = film
film = movie
"#;
    let diags = run(src)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn test_e0093_typeddict_missing_required_keys_in_literal() -> Result<(), Box<dyn std::error::Error>>
{
    let src = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int
    director: str

movie: Movie = {"name": "Blade Runner"}
"#;
    let diags = run(src)?;
    let e0093: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
        .collect();
    assert!(
        !e0093.is_empty(),
        "Missing required keys in dict literal should fire E0093"
    );
    Ok(())
}
