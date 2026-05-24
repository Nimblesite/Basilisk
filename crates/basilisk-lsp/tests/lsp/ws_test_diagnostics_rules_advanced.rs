//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Tests for LSP: `ws_test_diagnostics_rules_advanced`.

// WebSocket LSP E2E tests — Diagnostic rules E0026–E0054 and multi-rule pipeline.

use super::ws_test_common::*;

// ── E0026: TypeVar single constraint ────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0026_typevar_single_constraint_fires() -> TestResult<()> {
    let code = "\
from typing import TypeVar
T = TypeVar(\"T\", int)
";
    assert_rule_fires(
        "file:///e0026.py",
        code,
        "BSK-E0026",
        &["typevar", "constraint", "single"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0026_typevar_two_constraints_is_clean() -> TestResult<()> {
    let code = "\
from typing import TypeVar
T = TypeVar(\"T\", int, str)
";
    assert_rule_clean("file:///e0026_clean.py", code, "BSK-E0026").await
}

// ── E0027: Duplicate TypeVar in Generic ─────────────────────────────────────

#[tokio::test]
async fn test_ws_e0027_duplicate_typevar_fires() -> TestResult<()> {
    let code = "\
from typing import TypeVar, Generic
T = TypeVar(\"T\")

class Container(Generic[T, T]):
    pass
";
    assert_rule_fires("file:///e0027.py", code, "BSK-E0027", &[]).await
}

#[tokio::test]
async fn test_ws_e0027_unique_typevars_is_clean() -> TestResult<()> {
    let code = "\
from typing import TypeVar, Generic
T = TypeVar(\"T\")
U = TypeVar(\"U\")

class Container(Generic[T, U]):
    pass
";
    assert_rule_clean("file:///e0027_clean.py", code, "BSK-E0027").await
}

// ── E0034: @final decorator violations ──────────────────────────────────────

#[tokio::test]
async fn test_ws_e0034_inherit_from_final_class_fires() -> TestResult<()> {
    let code = "\
from typing import final

@final
class Sealed:
    pass

class Child(Sealed):
    pass
";
    assert_rule_fires(
        "file:///e0034.py",
        code,
        "BSK-E0034",
        &["final", "sealed", "inherit"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0034_final_on_module_function_fires() -> TestResult<()> {
    let code = "\
from typing import final

@final
def standalone() -> None:
    pass
";
    assert_rule_fires("file:///e0034_func.py", code, "BSK-E0034", &[]).await
}

#[tokio::test]
async fn test_ws_e0034_override_final_method_fires() -> TestResult<()> {
    let code = "\
from typing import final, override

class Base:
    @final
    def locked(self) -> str:
        return \"base\"

class Child(Base):
    @override
    def locked(self) -> str:
        return \"child\"
";
    assert_rule_fires("file:///e0034_method.py", code, "BSK-E0034", &[]).await
}

#[tokio::test]
async fn test_ws_e0034_final_method_not_overridden_is_clean() -> TestResult<()> {
    let code = "\
from typing import final

class Base:
    @final
    def locked(self) -> str:
        return \"base\"

class Child(Base):
    def other(self) -> str:
        return \"child\"
";
    assert_rule_clean("file:///e0034_clean.py", code, "BSK-E0034").await
}

// ── E0054: Final re-assignment ──────────────────────────────────────────────

#[tokio::test]
async fn test_ws_e0054_final_reassignment_fires() -> TestResult<()> {
    let code = "\
from typing import Final

RATE: Final = 3000

class Config:
    def update(self) -> None:
        global RATE
        RATE = 9999
";
    assert_rule_fires(
        "file:///e0054.py",
        code,
        "BSK-E0054",
        &["final", "rate", "reassign", "modify"],
    )
    .await
}

#[tokio::test]
async fn test_ws_e0054_final_class_attr_reassignment_fires() -> TestResult<()> {
    let code = "\
from typing import Final

class Config:
    MAX: Final[int] = 100

    def bad_update(self) -> None:
        self.MAX = 200
";
    assert_rule_fires("file:///e0054_class.py", code, "BSK-E0054", &[]).await
}

#[tokio::test]
async fn test_ws_e0054_final_not_reassigned_is_clean() -> TestResult<()> {
    let code = "\
from typing import Final

RATE: Final = 3000

def read_rate() -> int:
    return RATE
";
    assert_rule_clean("file:///e0054_clean.py", code, "BSK-E0054").await
}

// ── Full-pipeline: multiple rules in one file ───────────────────────────────

#[tokio::test]
async fn test_ws_multiple_rules_same_file() -> TestResult<()> {
    // This file intentionally triggers E0001, E0002, E0014, E0023
    let code = "\
count: int = \"wrong\"

def classify(x):
    match x:
        case 1:
            return \"one\"
";
    let (_fixture, raw) = open_and_diagnose("file:///multi_rules.py", code).await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;

    let diagnostics = json["params"]["diagnostics"]
        .as_array()
        .ok_or("no diagnostics array")?;

    assert!(
        !diagnostics.is_empty(),
        "multi-error file should fire diagnostics: {raw}"
    );

    // All diagnostics must have valid ranges
    for diag in diagnostics {
        assert_valid_range(diag, "multi-rule file");
    }

    // All diagnostics must have non-empty codes
    for diag in diagnostics {
        assert!(
            diag["code"].as_str().is_some_and(|c| !c.is_empty()),
            "every diagnostic must have a non-empty code: {diag}"
        );
    }

    // Must fire at least E0001 (unannotated param) and E0014 (int = "wrong")
    assert!(
        extract_diagnostic(&json, "BSK-E0001").is_some(),
        "should fire E0001 for unannotated param: {raw}"
    );
    assert!(
        extract_diagnostic(&json, "BSK-E0014").is_some(),
        "should fire E0014 for int = \"wrong\": {raw}"
    );
    assert!(
        extract_diagnostic(&json, "BSK-E0023").is_some(),
        "should fire E0023 for non-exhaustive match: {raw}"
    );
    Ok(())
}
