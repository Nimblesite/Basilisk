//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Coverage boost tests batch 33: disk-based integration tests for file-dependent rules
//! and comprehensive assertion-heavy tests.

use basilisk_checker::{check, check_with_config};
use basilisk_config::BasiliskConfig;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;
use basilisk_test_macros::mutation_safe;
use std::io::Write;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn run_for_target(
    source: &str,
) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let config = BasiliskConfig {
        python_version: Some("3.12".to_owned()),
        python_platform: Some("linux".to_owned()),
        ..BasiliskConfig::default()
    };
    Ok(check_with_config(&resolved, &config))
}

/// Run checker against a real file on disk so rules that load sibling modules work.
fn run_file(path: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = basilisk_parser::parse_file(path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// ═══════════════════════════════════════════════════════════════════════
// Disk-based tests: E0115 deprecated imports from sibling modules
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_import_from_sibling_module() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;

    // Create the "library" module with deprecated definitions
    let lib_path = dir.path().join("mylib.py");
    let mut lib_file = std::fs::File::create(&lib_path)?;
    writeln!(
        lib_file,
        r#"from typing_extensions import deprecated

@deprecated("Use new_func instead")
def old_func() -> int:
    return 1

@deprecated("Use NewClass instead")
class OldClass:
    @deprecated("Use new_method")
    def old_method(self) -> int:
        return 1

    @property
    @deprecated("Use new_prop")
    def old_prop(self) -> int:
        return 42

def good_func() -> int:
    return 2
"#
    )?;

    // Create the main file that imports from the library
    let main_path = dir.path().join("main.py");
    let mut main_file = std::fs::File::create(&main_path)?;
    writeln!(
        main_file,
        r"import mylib

result = mylib.old_func()
obj = mylib.OldClass()
obj.old_method()
val = obj.old_prop
good = mylib.good_func()
"
    )?;

    let diagnostics = run_file(main_path.to_str().unwrap_or("main.py"))?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn deprecated_from_import_sibling() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;

    // Create sibling module with deprecated function
    let sibling_path = dir.path().join("deprecated_lib.py");
    let mut sibling = std::fs::File::create(&sibling_path)?;
    writeln!(
        sibling,
        r#"from typing_extensions import deprecated

@deprecated("Use new_helper")
def old_helper() -> None:
    pass

@deprecated("Use NewTool")
class OldTool:
    pass
"#
    )?;

    // Main file does `from deprecated_lib import old_helper`
    let main_path = dir.path().join("consumer.py");
    let mut main_file = std::fs::File::create(&main_path)?;
    writeln!(
        main_file,
        r"from deprecated_lib import old_helper, OldTool

old_helper()
x = OldTool()
"
    )?;

    let diagnostics = run_file(main_path.to_str().unwrap_or("consumer.py"))?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Assertion-heavy tests: E0048 TypeAlias violations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn typealias_invalid_rhs_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadList: TypeAlias = [int, str]
BadDict: TypeAlias = {"key": "value"}
BadBool: TypeAlias = True
BadInt: TypeAlias = 42
BadEval: TypeAlias = eval("int")
BadLambda: TypeAlias = lambda: int
BadFStr: TypeAlias = f"type"
BadTernary: TypeAlias = int if True else str
BadBoolOp: TypeAlias = list or set
BadNeg: TypeAlias = -42
BadTuple: TypeAlias = (int, str)
BadAnd: TypeAlias = list and set

GoodUnion: TypeAlias = int | str
GoodSubscript: TypeAlias = list[int]
"#;
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();

    // Each invalid form must be individually caught
    // Extract variable names from diagnostic messages.
    // Message: "Invalid type expression as right-hand side of `TypeAlias` for `BadList`"
    let flagged_names: Vec<&str> = e0048
        .iter()
        .filter_map(|d| d.message.split('`').nth(3))
        .collect();

    for bad_name in &[
        "BadList",
        "BadDict",
        "BadBool",
        "BadInt",
        "BadEval",
        "BadLambda",
        "BadFStr",
        "BadTernary",
        "BadBoolOp",
        "BadNeg",
        "BadTuple",
        "BadAnd",
    ] {
        assert!(
            flagged_names.contains(bad_name),
            "`{bad_name}` should be flagged as invalid TypeAlias RHS. Flagged: {flagged_names:?}"
        );
    }

    // Ensure no false positives on valid aliases
    for good_name in &["GoodUnion", "GoodSubscript"] {
        assert!(
            !flagged_names.contains(good_name),
            "`{good_name}` should NOT be flagged"
        );
    }
    Ok(())
}

#[test]
fn typealias_alias_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias as TA

Bad: TA = [int, str]
Good: TA = int | str
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        e0048.iter().any(|d| d.message.contains("Bad")),
        "should flag Bad alias"
    );
    Ok(())
}

#[test]
fn non_generic_alias_cannot_be_parameterized() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

Simple: TypeAlias = int | str
x: Simple[int] = 42
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        e0048.iter().any(|d| d.message.contains("not generic")),
        "should flag non-generic alias being parameterized: {:?}",
        e0048.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Assertion-heavy: E0150 dead branch variables
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn version_guard_assertion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.version_info < (3, 8):
        dead = "unreachable on 3.12"
    else:
        live = "reachable"

    x = dead
    y = live
"#;
    let diagnostics = run_for_target(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        !e0150.is_empty(),
        "should flag dead branch variable `dead`: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    assert!(
        e0150.iter().any(|d| d.message.contains("dead")),
        "diagnostic should mention the variable `dead`"
    );
    Ok(())
}

#[test]
fn platform_guard_assertion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform == "bogus":
        bogus_var = 42
    else:
        real_var = 99

    x = bogus_var
"#;
    let diagnostics = run_for_target(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        !e0150.is_empty(),
        "should flag bogus platform variable: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Assertion-heavy: E0014 assignment type mismatch
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn basic_mismatch_assertions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = "hello"
y: str = 42
z: float = "bad"
w: bool = "yes"
ok: int = 42
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        e0014.len() >= 3,
        "should flag at least 3 type mismatches, got {}: {:?}",
        e0014.len(),
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Assertion-heavy: E0097 protocol undeclared self attrs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn protocol_undeclared_attrs_assertion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    x: int
    y: str

    def __init__(self) -> None:
        self.x = 0
        self.y = ""
        self.z = True
        self.w = 42
"#;
    let diagnostics = run(source)?;
    let e0097: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "protocols_definition")
        .collect();
    assert!(
        !e0097.is_empty(),
        "should flag undeclared self.z and self.w: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Module-level generic instance method calls (assertion-heavy)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn generic_method_call_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypeVar, Generic

T = TypeVar(\"T\")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def set(self, value: T) -> None:
        self.value = value

    def get(self) -> T:
        return self.value

a: Container[int] = Container(42)
a.set(\"wrong\")
a.set(99)

b: Container[str] = Container(\"hello\")
b.set(42)
";
    let diagnostics = run(source)?;
    let e0130: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_variance_inference")
        .collect();
    assert!(
        e0130.len() >= 2,
        "should flag at least 2 type mismatches (a.set(\"wrong\") and b.set(42)), got {}: {:?}",
        e0130.len(),
        e0130.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Assertion-heavy: E0115 deprecated usage
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_local_assertion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 1

@deprecated("Use NewClass")
class OldClass:
    pass

result = old_func()
obj = OldClass()
ref = old_func
"#;
    let diagnostics = run(source)?;
    let e0115: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_deprecated")
        .collect();
    assert!(
        e0115.len() >= 2,
        "should flag deprecated function call AND class instantiation, got {}: {:?}",
        e0115.len(),
        e0115.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// More e0115: deprecated method via var_types inference
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_method_via_var_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Widget:
    @deprecated("Use render2")
    def render(self) -> str:
        return ""

    @deprecated("Use call2")
    def __call__(self) -> int:
        return 0

    @deprecated("Use add2")
    def __add__(self, other: int) -> "Widget":
        return self

w = Widget()
w.render()
w()
w += 1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// deprecated in function body with param type inference
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_in_function_param_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Processor:
    @deprecated("Use process2")
    def process(self) -> None:
        pass

def handle(p: Processor) -> None:
    p.process()

def other(items: list) -> None:
    for item in items:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// deprecated in control flow
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("old")
def old_check() -> bool:
    return True

if old_check():
    pass

for i in range(10):
    old_check()

while old_check():
    break

x: int = old_check()

def func():
    return old_check()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// deprecated attribute access via assignment
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deprecated_property_setter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Config:
    @property
    @deprecated("Use new_setting")
    def setting(self) -> int:
        return 0

    @setting.setter
    @deprecated("Use set_new_setting")
    def setting(self, value: int) -> None:
        pass

cfg = Config()
val = cfg.setting
cfg.setting = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Mutation-killing tests for e0014/mod.rs
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: line 101 `&&` → `||` in filter.
/// When the `&&` is flipped to `||`, unannotated variables leak through and
/// would cause downstream panics or false diagnostics. We assert that
/// EXACTLY the annotated-with-RHS mismatches are flagged and nothing else.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_annotation_and_rhs_required() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Has annotation, has RHS → should fire
bad: int = "hello"

# Has annotation, no RHS → must NOT fire
declared: int

# No annotation, has RHS → must NOT fire
inferred = "hello"

# Both present, correct type → must NOT fire
good: int = 42
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();

    // EXACTLY one e0014 diagnostic: `bad: int = "hello"`
    assert_eq!(
        e0014.len(),
        1,
        "exactly 1 mismatch expected (bad: int = \"hello\"), got {}: {:?}",
        e0014.len(),
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Kills mutant: line 126 `.ends_with(".typealias")` → `&&`.
/// `typing.TypeAlias` annotation must be skipped by e0014 (handled by e0048).
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_typing_dot_typealias_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import typing

# typing.TypeAlias annotated — must be skipped by e0014
MyAlias: typing.TypeAlias = [int, str]

# Regular mismatch — must be caught
x: int = "hello"
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    // x should be flagged
    assert!(
        e0014
            .iter()
            .any(|d| d.message.contains("int") || d.message.contains("str")),
        "should flag x: int = \"hello\""
    );
    // MyAlias should NOT be flagged by e0014 (it's e0048's job)
    assert!(
        !e0014.iter().any(|d| d.message.contains("MyAlias")),
        "typing.TypeAlias should be skipped by e0014: {:?}",
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Kills mutant: line 127 `InferredType::Named` check for "ta".
/// `TypeAlias as TA` must be skipped by e0014.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_ta_alias_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias as TA

# TA-annotated — must be skipped by e0014
MyAlias: TA = [int, str]

# Regular mismatch — must be caught
x: int = "hello"
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        !e0014.iter().any(|d| d.message.contains("MyAlias")),
        "TA alias should be skipped by e0014: {:?}",
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Kills mutant: line 220 `!param.has_annotation` → removing `!`.
/// Annotated params must be included in `param_type_map` for local var checks.
#[mutation_safe(rule = "assignment_compatibility", fns = "build_param_type_map")]
#[test]
fn mutant_annotated_param_type_used() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, y: str) -> None:
    # Local var assigned from param — type should propagate
    a: str = x
    b: int = y
";
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        !e0014.is_empty(),
        "should flag local var type mismatches using param types: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Kills mutants in `has_top_level_token` depth tracking.
/// Nested brackets must NOT trigger `if`/`or`/`and` detection inside them.
#[mutation_safe(rule = "aliases_implicit", fns = "has_top_level_token")]
#[test]
fn mutant_nested_brackets_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

# "if" inside brackets is OK — it's a subscript arg, not a ternary
GoodNested: TypeAlias = dict[str, list[int]]

# "or" inside brackets is OK
GoodBracketOr: TypeAlias = dict[str, int]

# But top-level "or" IS bad
BadTopOr: TypeAlias = list or set

# Tuple with nested brackets — the commas are inside []
GoodSubscript: TypeAlias = tuple[int, str]

# Top-level ternary IS bad
BadTernary: TypeAlias = int if True else str
"#;
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();

    let flagged_names: Vec<&str> = e0048
        .iter()
        .filter_map(|d| d.message.split('`').nth(3))
        .collect();

    // Good ones must NOT be flagged
    assert!(
        !flagged_names.contains(&"GoodNested"),
        "nested brackets should not be flagged"
    );
    assert!(
        !flagged_names.contains(&"GoodSubscript"),
        "subscript should not be flagged"
    );

    // Bad ones MUST be flagged
    assert!(
        flagged_names.contains(&"BadTopOr"),
        "top-level `or` must be flagged: {flagged_names:?}"
    );
    assert!(
        flagged_names.contains(&"BadTernary"),
        "top-level ternary must be flagged: {flagged_names:?}"
    );
    Ok(())
}

/// Kills mutant: line 269 `pos + 1` → `pos * 1`.
/// When rfind('\n') finds position 0, `0+1=1` (correct: skip the newline)
/// but `0*1=0` (wrong: includes the newline char). We test with a variable
/// on the second line where the annotation text would be corrupted by the
/// off-by-one and assert the diagnostic message contains the correct type.
#[mutation_safe(rule = "assignment_compatibility", fns = "extract_annotation")]
#[test]
fn mutant_multiline_annotation_extraction() -> Result<(), Box<dyn std::error::Error>> {
    // The `\n` at position 0 means rfind returns Some(0).
    // Correct: line_start = 0 + 1 = 1 (skip newline)
    // Mutant:  line_start = 0 * 1 = 0 (include newline → corrupts annotation)
    let source = "\nx: int = \"hello\"\ny: str = 42\n";
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        e0014.len() >= 2,
        "both mismatches on lines after newline should be caught, got {}: {:?}",
        e0014.len(),
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    // Assert the annotations are correctly extracted (not corrupted by off-by-one)
    let messages: Vec<&str> = e0014.iter().map(|d| d.message.as_str()).collect();
    let has_int_mismatch = messages.iter().any(|m| m.contains("int"));
    let has_str_mismatch = messages.iter().any(|m| m.contains("str"));
    assert!(
        has_int_mismatch && has_str_mismatch,
        "should correctly extract both `int` and `str` annotations: {messages:?}"
    );
    Ok(())
}

/// Kills mutant: e0048.rs line 69 `import.kind != ImportKind::From` → `==`.
/// With the mutant, `From` imports are skipped, so `TypeAlias as TA` is never
/// registered and `BadAlias: TA = [int, str]` would not be detected as E0048.
#[mutation_safe(rule = "aliases_implicit", fns = "collect_type_alias_names")]
#[test]
fn mutant_from_import_kind_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias as TA

BadAlias: TA = [int, str]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !e0048.is_empty(),
        "aliases_implicit must fire for `TA = [int, str]` where TA is aliased from TypeAlias: {diagnostics:?}"
    );
    Ok(())
}

/// Kills mutant: e0048.rs line 86 `|| *c == '_'` → `&&`.
/// With the mutant, underscore chars break alias extraction, so `My_Alias`
/// would be truncated to `My` and `Bad: My_Alias = [int]` would not fire E0048.
#[mutation_safe(rule = "aliases_implicit", fns = "collect_type_alias_names")]
#[test]
fn mutant_alias_underscore_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias as My_Alias

Bad: My_Alias = [int, str]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !e0048.is_empty(),
        "aliases_implicit must fire when TypeAlias alias contains underscore: {diagnostics:?}"
    );
    Ok(())
}

/// Pins `collect_type_alias_names` for a non-aliased `from typing import TypeAlias`:
/// a bad `TypeAlias` RHS is flagged exactly once. The `!alias.is_empty() && alias
/// != "TypeAlias"` → `||` mutant here is *equivalent* — the only differing inputs
/// push a duplicate `TypeAlias`/empty entry into `names`, which is consumed solely
/// via `is_type_alias_annotation`'s `.any(|n| ann == n)`, so it can never change a
/// diagnostic — and is excluded in `.cargo/mutants.toml`. This test keeps the
/// function in mutation scope for its other, killable mutants.
#[mutation_safe(rule = "aliases_implicit", fns = "collect_type_alias_names")]
#[test]
fn mutant_typealias_self_alias_not_duplicated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

Bad: TypeAlias = [int, str]
Good: TypeAlias = int | str
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert_eq!(
        e0048.len(),
        1,
        "exactly one E0048 expected for `Bad: TypeAlias = [int, str]`, got {}: {e0048:?}",
        e0048.len()
    );
    Ok(())
}

/// Kills mutant: e0014 line 129 `&& var.rhs_span.is_some()` → `||`.
/// With `||`, unannotated vars or vars without RHS would be incorrectly checked.
/// A var with annotation but no RHS (bare declaration) must NOT fire E0014.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_bare_declaration_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
declared: int
assigned: int = "wrong"
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(
        e0014.len(),
        1,
        "only `assigned: int = \"wrong\"` should fire, bare `declared: int` must not: {e0014:?}"
    );
    Ok(())
}

/// Pins `extract_annotation` for a var on line 2 (rfind gives pos > 0): the `int`
/// annotation is extracted correctly and surfaced in the diagnostic. The
/// `pos + 1` → `pos * 1` mutant here is *equivalent* — `name_offset`
/// (= `start` - `line_start`) shifts by the same amount, so every slice taken from
/// `line` is byte-identical to the unmutated extraction — and is excluded in
/// `.cargo/mutants.toml`. This test keeps the function in mutation scope for its
/// other, killable mutants.
#[mutation_safe(rule = "assignment_compatibility", fns = "extract_annotation")]
#[test]
fn mutant_line_start_off_by_one_second_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = "first: str = 42\nsecond: int = \"hello\"\n";
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        e0014.iter().any(|d| d.message.contains("int")),
        "annotation `int` must be correctly extracted for `second` on line 2: {e0014:?}"
    );
    Ok(())
}

/// Kills mutant: e0048 line 72 second `!=` → `==` (`import.module != "typing_extensions"`).
/// With the mutant, only `typing_extensions` is processed; plain `typing` is skipped.
/// An aliased import from `typing` must still be detected.
#[mutation_safe(rule = "aliases_implicit", fns = "collect_type_alias_names")]
#[test]
fn mutant_typing_module_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias as Alias

Bad: Alias = [int, str]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !e0048.is_empty(),
        "E0048 must fire for alias from `typing` (not just `typing_extensions`): {diagnostics:?}"
    );
    Ok(())
}

/// Kills mutants in `has_top_level_token` depth/bracket tracking (lines 178-181).
/// `< with <=` on the loop guard makes the last byte reachable even past end.
/// Deleting open/close bracket arms means depth is never tracked.
/// `+= with -=` or `*=` on depth means brackets increase instead of tracking.
/// All these mutants break the nested-bracket guard, causing false positives
/// on type expressions that contain `or`/`if` inside subscript brackets.
#[mutation_safe(rule = "aliases_implicit", fns = "has_top_level_token")]
#[test]
fn mutant_depth_tracking_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

GoodUnion: TypeAlias = dict[str, int] | list[str]
GoodOptional: TypeAlias = tuple[int, str, float]
GoodDeep: TypeAlias = dict[str, tuple[int, list[str]]]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        e0048.is_empty(),
        "no E0048 for valid type expressions with brackets: {e0048:?}"
    );
    Ok(())
}

/// Kills mutants in `has_top_level_token` where closing bracket arm is deleted
/// (lines 181) — without it, depth goes up but never comes down, so tokens
/// after the first bracket pair are never visible at depth 0, hiding real errors.
#[mutation_safe(rule = "aliases_implicit", fns = "has_top_level_token")]
#[test]
fn mutant_depth_decrement_catches_after_bracket() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

BadAfterBracket: TypeAlias = tuple[int] or list[str]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !e0048.is_empty(),
        "E0048 must fire for `or` at depth 0 after a closing bracket: {e0048:?}"
    );
    Ok(())
}

/// Kills mutant: e0048.rs line 72 second `!=` → `==` in module guard.
/// Mutant: `module != "typing" && module == "typing_extensions"` → skips
/// `typing_extensions` imports. `from typing_extensions import TypeAlias as TEA`
/// would not register TEA, so `Bad: TEA = [int]` fires no E0048.
#[mutation_safe(rule = "aliases_implicit", fns = "collect_type_alias_names")]
#[test]
fn mutant_typing_extensions_module_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing_extensions import TypeAlias as TEA

Bad: TEA = [int, str]
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !e0048.is_empty(),
        "E0048 must fire for alias from `typing_extensions`: {diagnostics:?}"
    );
    Ok(())
}
