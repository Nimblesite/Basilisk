#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for LSP: `ws_test_diagnostics_rules`.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

// ── E0003: Missing variable type annotation ─────────────────────────────────

#[tokio::test]
async fn test_ws_e0003_missing_variable_type_fires() -> TestResult<()> {
    assert_rule_fires("file:///e0003.py", "items = []\n", "BSK-E0003", &["type"]).await
}

#[tokio::test]
async fn test_ws_e0003_annotated_empty_list_is_clean() -> TestResult<()> {
    assert_rule_clean(
        "file:///e0003_clean.py",
        "items: list[int] = []\n",
        "BSK-E0003",
    )
    .await
}

// ── E0011: Return type mismatch ─────────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0011_return_type_mismatch_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0011.py",
        "def count() -> str:\n    return 42\n",
        "BSK-E0011",
        &["return", "type", "mismatch"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0011_matching_return_type_is_clean() -> TestResult<()> {
    assert_rule_clean(
        "file:///e0011_clean.py",
        "def count() -> int:\n    return 42\n",
        "BSK-E0011",
    )
    .await
}

// ── E0012: Argument type mismatch ───────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0012_argument_type_mismatch_fires() -> TestResult<()> {
    let code = "\
def add(x: int, y: int) -> int:
    return x + y

result: int = add(\"hello\", \"world\")
";
    assert_rule_fires(
        "file:///e0012.py",
        code,
        "BSK-E0012",
        &["argument", "type", "int", "str"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0012_correct_args_is_clean() -> TestResult<()> {
    let code = "\
def add(x: int, y: int) -> int:
    return x + y

result: int = add(1, 2)
";
    assert_rule_clean("file:///e0012_clean.py", code, "BSK-E0012").await
}

// ── E0013: Return type mismatch (inferred) ──────────────────────────────────

#[tokio::test]
async fn test_ws_e0013_return_mismatch_fires() -> TestResult<()> {
    // Returns a string literal but annotated -> int
    // Either E0011 or E0013 should fire for this mismatch
    let code = "def label() -> int:\n    return \"hello\"\n";
    let (_fixture, raw) = open_and_diagnose("file:///e0013.py", code).await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;

    let fired = extract_diagnostic(&json, "BSK-E0013")
        .or_else(|| extract_diagnostic(&json, "BSK-E0011"))
        .ok_or("neither E0013 nor E0011 fired for return type mismatch")?;
    assert_valid_range(fired, "E0013/E0011");
    Ok(())
}

// ── E0014: Assignment type incompatibility ──────────────────────────────────

#[tokio::test]
async fn test_ws_e0014_assignment_type_mismatch_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0014.py",
        "count: int = \"hello\"\n",
        "BSK-E0014",
        &["int", "str", "type", "incompatible"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0014_multiple_type_mismatches() -> TestResult<()> {
    let code = "\
count: int = \"hello\"
label: str = 42
flag: bool = \"yes\"
ratio: float = \"1.5\"
";
    let (_fixture, raw) = open_and_diagnose("file:///e0014_multi.py", code).await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;

    let diagnostics = json["params"]["diagnostics"]
        .as_array()
        .ok_or("no diagnostics array")?;
    let e0014_count = diagnostics
        .iter()
        .filter(|d| d["code"].as_str() == Some("BSK-E0014"))
        .count();
    assert!(
        e0014_count >= 4,
        "expected >=4 E0014 diagnostics, got {e0014_count}: {raw}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_e0014_correct_assignments_are_clean() -> TestResult<()> {
    let code = "\
count: int = 42
label: str = \"hello\"
flag: bool = True
ratio: float = 1.5
";
    assert_rule_clean("file:///e0014_clean.py", code, "BSK-E0014").await
}

// ── E0015: Invalid type argument count ──────────────────────────────────────

#[tokio::test]
async fn test_ws_e0015_list_wrong_arg_count_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0015.py",
        "def f(x: list[int, str]) -> None:\n    pass\n",
        "BSK-E0015",
        &[],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0015_dict_wrong_arg_count_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0015_dict.py",
        "def f(x: dict[str]) -> None:\n    pass\n",
        "BSK-E0015",
        &[],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0015_correct_generics_are_clean() -> TestResult<()> {
    let code = "\
def f(x: list[int], y: dict[str, int], z: set[str]) -> None:
    pass
";
    assert_rule_clean("file:///e0015_clean.py", code, "BSK-E0015").await
}

// ── E0016: Incompatible method override ─────────────────────────────────────

#[tokio::test]
async fn test_ws_e0016_incompatible_override_fires() -> TestResult<()> {
    let code = "\
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: int) -> int:
        return data
";
    assert_rule_fires(
        "file:///e0016.py",
        code,
        "BSK-E0016",
        &["override", "incompatible", "process"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0016_compatible_override_is_clean() -> TestResult<()> {
    let code = "\
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> str:
        return data.upper()
";
    assert_rule_clean("file:///e0016_clean.py", code, "BSK-E0016").await
}

// ── E0017: Incompatible class attribute override ────────────────────────────

#[tokio::test]
async fn test_ws_e0017_attribute_override_fires() -> TestResult<()> {
    let code = "\
class Base:
    count: int = 0

class Child(Base):
    count: str = \"zero\"
";
    assert_rule_fires(
        "file:///e0017.py",
        code,
        "BSK-E0017",
        &["count", "override", "attribute", "int", "str"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0017_same_type_attribute_is_clean() -> TestResult<()> {
    let code = "\
class Base:
    count: int = 0

class Child(Base):
    count: int = 99
";
    assert_rule_clean("file:///e0017_clean.py", code, "BSK-E0017").await
}

// ── E0018: Undefined variable in return ─────────────────────────────────────

#[tokio::test]
async fn test_ws_e0018_undefined_variable_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0018.py",
        "def compute() -> int:\n    return undefined_name\n",
        "BSK-E0018",
        &["undefined", "undefined_name", "unbound"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0018_defined_variable_is_clean() -> TestResult<()> {
    assert_rule_clean(
        "file:///e0018_clean.py",
        "def compute() -> int:\n    result = 42\n    return result\n",
        "BSK-E0018",
    )
    .await
}

// ── E0019: Unbound variable on some paths ───────────────────────────────────

#[tokio::test]
async fn test_ws_e0019_unbound_on_some_paths_fires() -> TestResult<()> {
    let code = "\
def maybe_assign(flag: bool) -> int:
    if flag:
        result = 42
    return result
";
    assert_rule_fires(
        "file:///e0019.py",
        code,
        "BSK-E0019",
        &["unbound", "result", "path"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0019_always_assigned_is_clean() -> TestResult<()> {
    let code = "\
def always_assign(flag: bool) -> int:
    if flag:
        result = 42
    else:
        result = 0
    return result
";
    assert_rule_clean("file:///e0019_clean.py", code, "BSK-E0019").await
}

// ── E0020: Missing @overload implementation ─────────────────────────────────

#[tokio::test]
async fn test_ws_e0020_missing_overload_impl_fires() -> TestResult<()> {
    let code = "\
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...
";
    assert_rule_fires(
        "file:///e0020.py",
        code,
        "BSK-E0020",
        &["overload", "implementation", "process"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0020_overload_with_impl_is_clean() -> TestResult<()> {
    let code = "\
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    return x
";
    assert_rule_clean("file:///e0020_clean.py", code, "BSK-E0020").await
}

// ── E0021: Overlapping @overload signatures ─────────────────────────────────

#[tokio::test]
async fn test_ws_e0021_overlapping_overloads_fires() -> TestResult<()> {
    let code = "\
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: int) -> str: ...

def process(x: int) -> int | str:
    return x
";
    assert_rule_fires("file:///e0021.py", code, "BSK-E0021", &[]).await
}

// ── E0022: Unhashable dict key ──────────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0022_list_as_dict_key_fires() -> TestResult<()> {
    let code = "\
def bad_keys() -> None:
    mapping = {[1, 2]: \"value\"}
";
    assert_rule_fires(
        "file:///e0022.py",
        code,
        "BSK-E0022",
        &["hashable", "list", "key"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0022_set_as_dict_key_fires() -> TestResult<()> {
    let code = "\
def bad_keys() -> None:
    mapping = {{1, 2}: \"value\"}
";
    assert_rule_fires("file:///e0022_set.py", code, "BSK-E0022", &[]).await
}

#[tokio::test]
async fn test_ws_e0022_hashable_key_is_clean() -> TestResult<()> {
    let code = "\
def good_keys() -> None:
    mapping = {\"key\": \"value\", 42: \"number\", (1, 2): \"tuple\"}
";
    assert_rule_clean("file:///e0022_clean.py", code, "BSK-E0022").await
}

// ── E0023: Non-exhaustive match ─────────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0023_non_exhaustive_match_fires() -> TestResult<()> {
    let code = "\
def classify(x: int) -> str:
    match x:
        case 1:
            return \"one\"
        case 2:
            return \"two\"
    return \"other\"
";
    assert_rule_fires(
        "file:///e0023.py",
        code,
        "BSK-E0023",
        &["exhaustive", "match", "wildcard"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0023_exhaustive_match_is_clean() -> TestResult<()> {
    let code = "\
def classify(x: int) -> str:
    match x:
        case 1:
            return \"one\"
        case 2:
            return \"two\"
        case _:
            return \"other\"
";
    assert_rule_clean("file:///e0023_clean.py", code, "BSK-E0023").await
}

// ── E0024: Numeric literal as type annotation ───────────────────────────────

#[tokio::test]
async fn test_ws_e0024_literal_as_annotation_fires() -> TestResult<()> {
    assert_rule_fires(
        "file:///e0024.py",
        "def f(x: 42) -> 0:\n    pass\n",
        "BSK-E0024",
        &["literal", "annotation", "type"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0024_real_type_annotation_is_clean() -> TestResult<()> {
    assert_rule_clean(
        "file:///e0024_clean.py",
        "def f(x: int) -> str:\n    return str(x)\n",
        "BSK-E0024",
    )
    .await
}

// ── E0025: Missing @override decorator ──────────────────────────────────────

#[tokio::test]
async fn test_ws_e0025_missing_override_decorator_fires() -> TestResult<()> {
    let code = "\
class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    def process(self, data: str) -> str:
        return data.upper()
";
    assert_rule_fires(
        "file:///e0025.py",
        code,
        "BSK-E0025",
        &["override", "process", "decorator"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0025_with_override_decorator_is_clean() -> TestResult<()> {
    let code = "\
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> str:
        return data.upper()
";
    assert_rule_clean("file:///e0025_clean.py", code, "BSK-E0025").await
}
