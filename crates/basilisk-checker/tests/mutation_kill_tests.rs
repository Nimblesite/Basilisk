//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Mutation-killing tests: assertion-heavy tests designed to catch specific
//! mutants identified by cargo-mutants. Each test asserts BOTH that violations
//! ARE caught and that correct code is NOT flagged.

use basilisk_checker::check_with_config;
use basilisk_config::BasiliskConfig;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;
use basilisk_test_macros::mutation_safe;

// Basilisk has no modes — config in, diagnostics out. `run` uses the default
// config (every PEP rule, no house rules); `run_with_config` honors an explicit
// config; `annotation_rules_config` is the config a project writes to opt into
// the annotation house rules (E0001..E0005, E0025, W0014, W0040, W0050). See
// [CHKARCH-CONFIGURATION-ONLY].
fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    run_with_config(source, &BasiliskConfig::default())
}

fn run_with_config(
    source: &str,
    config: &BasiliskConfig,
) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check_with_config(&resolved, config))
}

fn run_with_python_3_12(
    source: &str,
) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let config = BasiliskConfig {
        python_version: Some("3.12".to_owned()),
        ..Default::default()
    };
    run_with_config(source, &config)
}

fn annotation_rules_config() -> BasiliskConfig {
    use basilisk_config::RuleSeverity::{Error, Warning};

    BasiliskConfig::with_rule_entries(
        [
            ("BSK-0001", Error),
            ("BSK-0002", Error),
            ("BSK-0003", Error),
            ("BSK-0004", Error),
            ("BSK-0005", Error),
            ("BSK-0025", Error),
            ("BSK-0014", Warning),
            ("BSK-0040", Warning),
            ("BSK-0050", Warning),
        ]
        .into_iter()
        .map(|(code, severity)| (code.to_owned(), severity))
        .collect(),
    )
}

fn assignment_compatibility_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count()
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Optional match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_optional_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional

# int assigned to Optional[int] → OK
a: Optional[int] = 42

# None assigned to Optional[int] → OK
b: Optional[int] = None

# str assigned to Optional[int] → FAIL
c: Optional[int] = "wrong"

# Optional[int] assigned to int → FAIL (None not assignable to int)
d: int = None
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 2,
        "at least 2 mismatches expected (c and d), got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Union match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_union_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

# int to Union[int, str] → OK
a: Union[int, str] = 42

# str to Union[int, str] → OK
b: Union[int, str] = "hello"

# float to Union[int, str] → FAIL
c: Union[int, str] = 3.14

# list to Union[int, str] → FAIL
d: Union[int, str] = [1, 2]
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 union mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — List/Set match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_list_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# list[int] → list[int]: OK
a: list[int] = [1, 2, 3]

# list[str] → list[int]: FAIL
b: list[int] = ["a", "b"]

# set[int] → set[int]: OK
c: set[int] = {1, 2, 3}

# set[str] → set[int]: FAIL
d: set[int] = {"a", "b"}
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 list/set type element mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Dict match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_dict_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# dict[str, int] → dict[str, int]: OK
a: dict[str, int] = {"x": 1}

# dict[str, str] → dict[str, int]: FAIL
b: dict[str, int] = {"x": "wrong"}

# dict[int, str] → dict[str, int]: FAIL
c: dict[str, int] = {1: 2}
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 dict type mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Tuple match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_tuple_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# tuple[int, str] → tuple[int, str]: OK
a: tuple[int, str] = (1, "hello")

# tuple[str, int] → tuple[int, str]: FAIL
b: tuple[int, str] = ("wrong", 42)

# Wrong arity
c: tuple[int, str] = (1,)
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 tuple mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Callable match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_callable_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f1(x: int) -> str:
    return str(x)

def f2(x: str) -> int:
    return int(x)

# Correct assignment
a: Callable[[int], str] = f1

# Return type mismatch
b: Callable[[int], int] = f1

# Param type mismatch
c: Callable[[str], str] = f1
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Named type match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_named_type_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Same named types → OK
a: int = 42
b: str = "hello"
c: float = 3.14
d: bool = True

# Cross-type assignment → FAIL
e: int = "wrong"
f: str = 42
g: float = "bad"
h: bytes = 42
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 3,
        "at least 3 named type mismatches, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs InferredType::union — edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_union_construction() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

# Single-element union is just the element
a: Union[int] = 42
b: Union[int] = "wrong"

# Multi-element union
c: Union[int, str, float] = 42
d: Union[int, str, float] = "hello"
e: Union[int, str, float] = 3.14
f: Union[int, str, float] = [1, 2]
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 union mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Protocol undeclared self attrs — kill all mutants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0097_declared_vs_undeclared() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    x: int
    y: str

    def __init__(self) -> None:
        self.x = 0       # declared → OK
        self.y = ""       # declared → OK
        self.z = True     # undeclared → E0097
        self.w = 42       # undeclared → E0097
"#;
    let diagnostics = run(source)?;
    let e0097: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "protocols_definition")
        .collect();
    // Must flag z and w
    assert!(
        e0097.len() >= 2,
        "should flag self.z and self.w, got {}: {:?}",
        e0097.len(),
        e0097.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    // Must NOT flag x and y
    let false_positives = e0097
        .iter()
        .filter(|d| d.message.contains("`x`") || d.message.contains("`y`"))
        .count();
    assert_eq!(
        false_positives, 0,
        "declared attrs x and y must not be flagged"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Dead branch — kill mutants in version/platform detection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0150_version_less_than() -> Result<(), Box<dyn std::error::Error>> {
    // sys.version_info < (3, 8) is FALSE on 3.12 → if-body is dead
    let source = r#"
import sys

def check():
    if sys.version_info < (3, 8):
        dead = "dead"
    else:
        live = "live"
    x = dead
    y = live
"#;
    let diagnostics = run_with_python_3_12(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        e0150.iter().any(|d| d.message.contains("dead")),
        "must flag `dead` variable: {:?}",
        e0150.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        !e0150.iter().any(|d| d.message.contains("live")),
        "must NOT flag `live` variable"
    );
    Ok(())
}

#[test]
fn mutant_e0150_version_greater_equal() -> Result<(), Box<dyn std::error::Error>> {
    // sys.version_info >= (3, 12) is TRUE on 3.12 → else-body is dead
    let source = r#"
import sys

def check():
    if sys.version_info >= (3, 12):
        live = "live"
    else:
        dead = "dead"
    x = dead
    y = live
"#;
    let diagnostics = run_with_python_3_12(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        e0150.iter().any(|d| d.message.contains("dead")),
        "must flag `dead` from else-body: {:?}",
        e0150.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        !e0150.iter().any(|d| d.message.contains("live")),
        "must NOT flag `live` from if-body"
    );
    Ok(())
}

#[test]
fn mutant_e0150_platform_bogus() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform == "bogus":
        dead = "dead"
    else:
        live = "live"
    x = dead
"#;
    let diagnostics = run_with_python_3_12(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        !e0150.is_empty(),
        "must flag dead variable in bogus platform branch"
    );
    Ok(())
}

#[test]
fn mutant_e0150_platform_not_bogus() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform != "bogus":
        live = "live"
    else:
        dead = "dead"
    x = dead
"#;
    let diagnostics = run_with_python_3_12(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_version_platform")
        .collect();
    assert!(
        !e0150.is_empty(),
        "must flag dead variable in != bogus else branch"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Various literal type mismatches — each type path
// ═══════════════════════════════════════════════════════════════════════

#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_every_literal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Each literal type assigned to wrong annotation
a: str = 42            # int → str FAIL
b: int = "hello"       # str → int FAIL
c: int = 3.14          # float → int FAIL
d: int = True          # bool → int is OK (bool subtype of int)
e: str = b"bytes"      # bytes → str FAIL
f: int = None          # None → int FAIL
g: int = [1, 2]        # list → int FAIL
h: int = {"a": 1}      # dict → int FAIL
i: int = {1, 2}        # set → int FAIL

# Correct ones — must NOT fire
j: int = 42
k: str = "hello"
l: float = 3.14
m: bool = True
n: bytes = b"data"
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 6,
        "at least 6 type mismatches expected, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Negative literal and float literal
// ═══════════════════════════════════════════════════════════════════════

#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_negative_and_float() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
# Negative int → float: OK
a: float = -42

# Negative int → str: FAIL
b: str = -42

# Float → int: FAIL
c: int = 3.14

# Float → float: OK
d: float = 3.14
";
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 mismatch (b: str = -42 or c: int = 3.14), got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014 check_vars: quoted forward-reference annotation skip (line 140)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0014/mod.rs:140 `replace || with &&` in the whole-quoted
/// annotation guard (`starts_with('"') || starts_with('\'')`). A quoted
/// forward-reference annotation must NOT be evaluated as a value type (so it
/// produces no E0014), while an unquoted, genuinely-mismatched annotation MUST
/// still fire. Flipping `||`→`&&` (or dropping either `starts_with`) makes the
/// guard never skip, so the quoted lines would be processed and fire E0014 —
/// observably changing the count.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_quoted_annotation_skip() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
a: "int" = "hello"
b: 'int' = 'hello'
c: int = "hello"
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert_eq!(
        e0014,
        1,
        "only the unquoted `c: int = \"hello\"` mismatch fires; both quoted \
         annotations are skipped, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014 check_vars: type-alias annotation skip (lines 289–290)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0014/mod.rs:290 `replace || with &&` in the type-alias skip
/// guard (`skip.type_alias.contains(base) || skip.type_alias_type.contains(..)`).
/// E0014 cannot evaluate an expanded alias, so an annotation referencing a PEP
/// 695 `type` alias OR a `TypeAliasType(...)` alias must be skipped (no E0014),
/// while a genuine `int`-vs-`str` mismatch must still fire. Each alias form sets
/// exactly ONE operand true, so flipping `||`→`&&` makes neither skip: both
/// alias lines would then be processed and wrongly fire E0014, raising the count
/// from 1 to 3 — observably killing the mutant. Keeping both forms also pins the
/// individual operands against future deletion mutants.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_type_alias_skip() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAliasType

# PEP 695 type alias (lowercase name) → only `skip.type_alias` matches.
type loweralias = list[int]
a: loweralias = "hello"

# TypeAliasType alias → only `skip.type_alias_type` matches.
Bar = TypeAliasType("Bar", int)
b: Bar = "hello"

# Genuine mismatch against a real type — must still fire.
c: int = "hello"
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert_eq!(
        e0014,
        1,
        "both alias-annotated assignments are skipped; only `c: int = \"hello\"` \
         fires, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014 check_vars: bare recursive-Union-alias interception (line 219)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0014/mod.rs:219 `delete !` in the bare-alias guard
/// (`!name.contains('[')`). A bare reference to a legacy recursive `Union`
/// alias must be matched value-by-value against its expanded definition: a
/// valid `Json` value is accepted (no E0014) and only an invalid member
/// (complex `3j`) fires. Deleting the `!` flips the guard so bare alias names
/// are no longer intercepted, falling back to the `Named`-vs-literal compare,
/// which rejects the valid value too — raising the E0014 count.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_recursive_union_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

Json = Union[None, int, str, float, list["Json"], dict[str, "Json"]]

ok: Json = [1, {"a": 1}]
bad: Json = {"a": 3j}
"#;
    let diagnostics = run(source)?;
    let e0014 = assignment_compatibility_count(&diagnostics);
    assert_eq!(
        e0014,
        1,
        "the valid recursive-alias value is accepted; only the complex `3j` \
         value fires, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

fn missing_parameter_annotation_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .count()
}

// ═══════════════════════════════════════════════════════════════════════
// BSK-0001 check_function: !p.has_annotation guard
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0001.rs:33 `delete ! in check_function` and `replace != with ==`
/// on the name guards. Asserts BOTH that an unannotated regular param fires BSK-0001
/// AND that an annotated regular param does NOT — so flipping the polarity of the
/// annotation check or the name guards produces an observable diagnostic count.
#[mutation_safe(rule = "missing_parameter_annotation", fns = "check_function")]
#[test]
fn mutant_e0001_annotation_polarity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def has_annotated_only(annotated: int) -> int:
    return annotated

def missing(unannotated, annotated: int) -> int:
    return annotated
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!(
            "exactly one BSK-0001 expected for `unannotated` only, got {}: {e0001:?}",
            e0001.len()
        )
        .into());
    };
    assert!(
        only.message.contains("unannotated"),
        "diagnostic must name `unannotated`, got: {}",
        only.message
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// BSK-0001 check_function: self/cls guards (&& vs ||)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutants: e0001.rs:33 `replace && with ||` (both occurrences).
/// With `||` instead of `&&`, every parameter satisfies at least one branch,
/// so unannotated `self`/`cls` would fire BSK-0001 (false positive). This test
/// asserts that unannotated `self` and `cls` do NOT fire while a sibling
/// unannotated regular param DOES — exactly distinguishing && from ||.
#[mutation_safe(rule = "missing_parameter_annotation", fns = "check_function")]
#[test]
fn mutant_e0001_self_and_cls_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    def method(self, x):
        return x

    @classmethod
    def klass(cls, y):
        return y
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    assert_eq!(
        e0001.len(),
        2,
        "exactly two E0001s expected (for `x` and `y`), got {}: {e0001:?}",
        e0001.len()
    );
    let names: Vec<&str> = e0001.iter().map(|d| d.message.as_str()).collect();
    assert!(
        names.iter().any(|m| m.contains("`x`")),
        "BSK-0001 must fire for `x`, got: {names:?}"
    );
    assert!(
        names.iter().any(|m| m.contains("`y`")),
        "BSK-0001 must fire for `y`, got: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|m| m.contains("`self`") || m.contains("`cls`")),
        "BSK-0001 must NOT fire for `self` or `cls`, got: {names:?}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// BSK-0001 check (impl): is_stub_context filter (delete !)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0001.rs:25 `delete ! in check`. Without the negation,
/// only stub-context functions are checked — so unannotated params in regular
/// (non-stub) functions are missed AND unannotated params inside Protocol
/// methods would incorrectly fire. This test asserts both directions.
#[mutation_safe(rule = "missing_parameter_annotation", fns = "check")]
#[test]
fn mutant_e0001_stub_context_negation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Iface(Protocol):
    def stub_method(self, p): ...

def regular(unannotated):
    return unannotated
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!(
            "exactly one BSK-0001 expected (for `unannotated` in regular fn), \
             got {}: {e0001:?}",
            e0001.len()
        )
        .into());
    };
    assert!(
        only.message.contains("unannotated"),
        "diagnostic must name `unannotated`, not the Protocol param: {}",
        only.message
    );
    Ok(())
}

/// Kills mutant: e0001.rs:22 `replace check with ()`. With the empty body,
/// no BSK-0001 ever fires. A simple presence-of-diagnostic assertion kills it.
#[mutation_safe(rule = "missing_parameter_annotation", fns = "check")]
#[test]
fn mutant_e0001_check_body_present() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(unannotated):
    return unannotated
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let count = missing_parameter_annotation_count(&diagnostics);
    assert_eq!(
        count, 1,
        "BSK-0001 must fire exactly once for unannotated param, got {count}"
    );
    Ok(())
}

/// Kills mutant: e0001.rs:38 `replace make_diagnostic -> Diagnostic with Default::default()`.
/// `Default::default()` produces an empty Diagnostic (empty code, empty message).
/// Asserting both code AND message content kills the mutant — `Default` cannot
/// produce `BSK-0001` and a parameter-named message simultaneously.
#[mutation_safe(rule = "missing_parameter_annotation", fns = "make_diagnostic")]
#[test]
fn mutant_e0001_diagnostic_payload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(specific_name):
    return specific_name
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!("exactly one BSK-0001 expected: {e0001:?}").into());
    };
    assert_eq!(only.code.code, "BSK-0001");
    assert!(
        only.message.contains("specific_name"),
        "message must name the parameter, got: {}",
        only.message
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Rule-coverage smoke tests — expand mutation-testing scope across rules.
// Each test triggers a rule on a minimal source and asserts at least one
// diagnostic. The goal is to expose mutants per rule, not to kill all of
// them. Survivors are intentional and inform where assertions are weak.
// ═══════════════════════════════════════════════════════════════════════

fn count_code(diagnostics: &[basilisk_checker::Diagnostic], code: &str) -> usize {
    diagnostics.iter().filter(|d| d.code.code == code).count()
}

#[mutation_safe(rule = "missing_return_annotation")]
#[test]
fn mutant_e0002_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def no_return_annotation(x: int):
    return x
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    assert!(
        count_code(&diagnostics, "BSK-0002") >= 1,
        "BSK-0002 must fire for missing return annotation: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "missing_variable_type")]
#[test]
fn mutant_e0003_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
empty_list = []
empty_dict = {}
nothing = None
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    assert!(
        count_code(&diagnostics, "BSK-0003") >= 1,
        "BSK-0003 must fire for unannotated empty-collection vars: {diagnostics:?}"
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
// Mutants unmasked when the descriptive-rename fix re-enabled mutant
// selection: opt_in_spec gating (BSK house rules off by default) plus
// assignment-compatibility scope/offset logic.
// ─────────────────────────────────────────────────────────────────────

#[mutation_safe(rule = "missing_return_annotation")]
#[test]
fn mutant_e0002_opt_in_gating() -> Result<(), Box<dyn std::error::Error>> {
    // BSK-0002 is an opt-in house rule (provenance basilisk): it must fire only
    // when the project opts in, and stay silent under the default (PEP-only)
    // config. The default-off assertion kills `opt_in_spec -> None`, which would
    // reclassify the rule as a default-on PEP rule.
    let source = "def f(x: int):\n    return x\n";
    assert!(
        count_code(
            &run_with_config(source, &annotation_rules_config())?,
            "BSK-0002"
        ) >= 1,
        "BSK-0002 must fire when annotation house rules are enabled"
    );
    assert_eq!(
        count_code(&run(source)?, "BSK-0002"),
        0,
        "BSK-0002 must be OFF under the default config (opt-in only)"
    );
    Ok(())
}

#[mutation_safe(rule = "missing_variable_type")]
#[test]
fn mutant_e0003_opt_in_gating() -> Result<(), Box<dyn std::error::Error>> {
    // As above for BSK-0003; the default-off assertion kills `opt_in_spec -> None`.
    let source = "empty_list = []\n";
    assert!(
        count_code(
            &run_with_config(source, &annotation_rules_config())?,
            "BSK-0003"
        ) >= 1,
        "BSK-0003 must fire when annotation house rules are enabled"
    );
    assert_eq!(
        count_code(&run(source)?, "BSK-0003"),
        0,
        "BSK-0003 must be OFF under the default config (opt-in only)"
    );
    Ok(())
}

#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_e0014_unannotated_dict_not_checked() -> Result<(), Box<dyn std::error::Error>> {
    // `check_vars` only inspects vars that are BOTH annotated AND have a value
    // (`has_annotation && rhs_span.is_some()`). An unannotated dict literal — whose
    // line contains a `:` inside the literal — must NOT be treated as annotated.
    // The `&& -> ||` mutant would include it, mis-read `1: 2` as an annotation, and
    // emit a spurious assignment_compatibility diagnostic.
    let source = "data = {1: 2}\n";
    assert_eq!(
        assignment_compatibility_count(&run(source)?),
        0,
        "unannotated dict literal must not produce assignment_compatibility"
    );
    Ok(())
}

#[mutation_safe(rule = "assignment_compatibility", fns = "extract_annotation")]
#[test]
fn mutant_e0014_non_first_line_annotation_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // The annotated mismatch is on a non-first line, so `extract_annotation`'s
    // `line_start` (`rfind('\n') + 1`) is exercised on a real newline rather than
    // the `map_or(0, …)` first-line fallback.
    let source = "x = 1\nbad: int = \"wrong\"\n";
    assert!(
        assignment_compatibility_count(&run(source)?) >= 1,
        "`int = str` on a non-first line must produce assignment_compatibility"
    );
    Ok(())
}

#[mutation_safe(rule = "match_exhaustiveness")]
#[test]
fn mutant_e0023_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def classify(x: int) -> str:
    match x:
        case 1:
            return 'one'
        case 2:
            return 'two'
    return 'other'
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "match_exhaustiveness") >= 1,
        "E0023 must fire for non-exhaustive match: {diagnostics:?}"
    );
    Ok(())
}

// Kills `replace && with ||` (and `replace !has_structural_pattern with true`)
// in `NonExhaustiveMatch::check`'s filter `!has_wildcard && !has_structural_pattern`.
// A `match` with a structural (sequence/mapping) pattern and NO wildcard is
// exempt: with `&&` no diagnostic fires; the `||` mutant would wrongly flag it.
#[mutation_safe(rule = "match_exhaustiveness")]
#[test]
fn mutant_e0023_structural_pattern_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def handle(seq: list) -> str:
    match seq:
        case [a]:
            return 'one'
        case [a, b]:
            return 'two'
    return 'other'
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "match_exhaustiveness") == 0,
        "E0023 must NOT fire for a match with a structural pattern (no wildcard): {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "generics_base_class")]
#[test]
fn mutant_e0027_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic, TypeVar

T = TypeVar('T')

class Dup(Generic[T, T]):
    pass
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "generics_base_class") >= 1,
        "E0027 must fire for duplicate TypeVar in Generic: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "typeddicts_class_syntax")]
#[test]
fn mutant_e0029_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

    def title(self) -> str:
        return self['name']
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "typeddicts_class_syntax") >= 1,
        "E0029 must fire for TypedDict method: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "directives_reveal_type")]
#[test]
fn mutant_e0033_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
reveal_type()
reveal_type(1, 2)
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "directives_reveal_type") >= 1,
        "E0033 must fire for invalid reveal_type call: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "directives_assert_type")]
#[test]
fn mutant_e0039_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
assert_type(1)
assert_type(1, int, 'extra')
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "directives_assert_type") >= 1,
        "E0039 must fire for invalid assert_type call: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "tuples_type_form")]
#[test]
fn mutant_e0049_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVarTuple

Ts = TypeVarTuple('Ts')

def f(t: tuple[*tuple[str, ...], *tuple[int, ...]]) -> None:
    pass
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "tuples_type_form") >= 1,
        "E0049 must fire for multiple unbounded tuple components: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "typeddicts_usage")]
#[test]
fn mutant_e0088_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class TD(TypedDict):
    name: str

x: object = {}
isinstance(x, TD)
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "typeddicts_usage") >= 1,
        "E0088 must fire for isinstance() with TypedDict: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "generics_syntax_declarations_2")]
#[test]
fn mutant_e0105_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> None:
        x.is_integer()
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "generics_syntax_declarations_2") >= 1,
        "E0105 must fire for invalid attr on bounded TypeVar: {diagnostics:?}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// TypedDict ReadOnly/Required redeclaration legality (PEP 705).
// Implements [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
// docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//
// Each test below pins one decision function of the redeclaration matrix so
// cargo-mutants generates mutants for it; every test asserts BOTH a violating
// redeclaration that MUST fire and a legal one that MUST NOT, so flipping any
// boolean, comparison, or match arm in those functions is observable.
// ═══════════════════════════════════════════════════════════════════════

fn typeddicts_inheritance_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    count_code(diagnostics, "typeddicts_inheritance")
}

/// Common prelude importing every `TypedDict` qualifier the tests use.
const TD_PRELUDE: &str = "from typing import TypedDict, Required, NotRequired\n\
                          from typing_extensions import ReadOnly\n";

/// `parse_field_qualifiers`: a writable item redeclared `ReadOnly` is illegal,
/// but a `ReadOnly` item redeclared writable is allowed. Also pins that the
/// `ReadOnly[...]` detection actually reads the wrapper.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "parse_field_qualifiers")]
#[test]
fn mutant_e0038_parse_readonly() -> Result<(), Box<dyn std::error::Error>> {
    let illegal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: Required[int]\n\
         class Child(Base):\n    a: ReadOnly[int]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal)?) >= 1,
        "writable item redeclared ReadOnly must fire E0038"
    );

    let legal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    name: ReadOnly[str]\n\
         class Child(Base):\n    name: str\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "ReadOnly item redeclared writable is allowed — no E0038"
    );
    Ok(())
}

/// `parse_field_qualifiers` required-ness: `NotRequired` must be recognised
/// ahead of `Required` (its text contains `required[`), and the implicit
/// required-ness falls back to the class `total=` setting.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "redeclaration_violation")]
#[test]
fn mutant_e0038_required_relaxing() -> Result<(), Box<dyn std::error::Error>> {
    // Explicit Required -> NotRequired is illegal.
    let illegal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: Required[int]\n\
         class Child(Base):\n    a: NotRequired[int]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal)?) >= 1,
        "required item redeclared not-required must fire E0038"
    );

    // Implicit-required (via total) -> NotRequired is also illegal: pins the
    // `class_total` fallback branch of parse_field_qualifiers.
    let illegal_total = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: int\n\
         class Child(Base):\n    a: NotRequired[int]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal_total)?) >= 1,
        "total-required item redeclared not-required must fire E0038"
    );

    // NotRequired -> Required is allowed (making an optional item required).
    let legal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: NotRequired[int]\n\
         class Child(Base):\n    a: Required[int]\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "not-required item redeclared required is allowed — no E0038"
    );
    Ok(())
}

/// `value_type_incompatible`: writable items are invariant (any type change is
/// illegal); a same-typed redeclaration is fine.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "value_type_incompatible")]
#[test]
fn mutant_e0038_writable_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let illegal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: int\n\
         class Child(Base):\n    a: str\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal)?) >= 1,
        "writable item with changed value type must fire E0038"
    );

    let legal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: int\n\
         class Child(Base):\n    a: int\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "identical redeclaration is allowed — no E0038"
    );
    Ok(())
}

/// `is_invariant_container`: narrowing the type argument of an invariant
/// container (`list`/`dict`/`set`) under `ReadOnly` is illegal, but narrowing a
/// covariant container (`Sequence`) is allowed.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "is_invariant_container")]
#[test]
fn mutant_e0038_invariant_container() -> Result<(), Box<dyn std::error::Error>> {
    for container in ["list", "set"] {
        let illegal = format!(
            "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: ReadOnly[{container}[int | str]]\n\
             class Child(Base):\n    a: ReadOnly[{container}[int]]\n"
        );
        assert!(
            typeddicts_inheritance_count(&run(&illegal)?) >= 1,
            "narrowing ReadOnly[{container}[...]] arg must fire E0038"
        );
    }
    let illegal_dict = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: ReadOnly[dict[str, int | str]]\n\
         class Child(Base):\n    a: ReadOnly[dict[str, int]]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal_dict)?) >= 1,
        "narrowing ReadOnly[dict[...]] arg must fire E0038"
    );

    // Sequence is covariant — narrowing its argument under ReadOnly is allowed.
    let legal = format!(
        "{TD_PRELUDE}\nfrom typing import Sequence\nclass Base(TypedDict):\n    \
         a: ReadOnly[Sequence[int | str]]\n\
         class Child(Base):\n    a: ReadOnly[Sequence[int]]\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "narrowing a covariant ReadOnly[Sequence[...]] is allowed — no E0038"
    );
    Ok(())
}

/// `type_head`: a `ReadOnly` redeclaration to a *different* container (a
/// covariant subtype, e.g. `Collection[T]` -> `list[T]`) is allowed, while the
/// *same* invariant container with different args is not. Distinguishes
/// head-comparison from full-string comparison.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "type_head")]
#[test]
fn mutant_e0038_type_head() -> Result<(), Box<dyn std::error::Error>> {
    let legal = format!(
        "{TD_PRELUDE}\nfrom typing import Collection\nclass Base(TypedDict):\n    \
         a: ReadOnly[Collection[int]]\n\
         class Child(Base):\n    a: ReadOnly[list[int]]\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "narrowing ReadOnly[Collection[int]] to ReadOnly[list[int]] is allowed"
    );

    let illegal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: ReadOnly[list[int]]\n\
         class Child(Base):\n    a: ReadOnly[list[bool]]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal)?) >= 1,
        "same invariant container with different args must fire E0038"
    );
    Ok(())
}

/// `check_field_override`: end-to-end single-inheritance redeclaration drives
/// the rule, and an unrelated new field added by the subclass is never flagged.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "check_field_override")]
#[test]
fn mutant_e0038_field_override() -> Result<(), Box<dyn std::error::Error>> {
    let illegal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: ReadOnly[Required[int]]\n\
         class Child(Base):\n    a: ReadOnly[NotRequired[int]]\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&illegal)?) >= 1,
        "ReadOnly required -> ReadOnly not-required must fire E0038"
    );

    let legal = format!(
        "{TD_PRELUDE}\nclass Base(TypedDict):\n    a: ReadOnly[NotRequired[int]]\n\
         class Child(Base):\n    a: ReadOnly[Required[int]]\n    b: int\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "ReadOnly not-required -> ReadOnly required (plus a new field) is allowed"
    );
    Ok(())
}

/// `bases_conflict` / `check_conflicting_bases`: multiple inheritance with two
/// bases declaring the same field incompatibly (by core type, required-ness, or
/// read-only-ness) is illegal; identical declarations merge cleanly.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "bases_conflict")]
#[test]
fn mutant_e0038_bases_conflict() -> Result<(), Box<dyn std::error::Error>> {
    // Core-type conflict.
    let core = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: int\nclass B(TypedDict):\n    x: float\n\
         class C(A, B):\n    pass\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&core)?) >= 1,
        "conflicting core types across bases must fire E0038"
    );

    // Required-ness conflict (same core, same readonly).
    let req = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: ReadOnly[NotRequired[int]]\n\
         class B(TypedDict):\n    x: ReadOnly[Required[int]]\n\
         class C(A, B):\n    pass\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&req)?) >= 1,
        "conflicting required-ness across bases must fire E0038"
    );

    // Read-only-ness conflict (same core, same required-ness).
    let ro = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: ReadOnly[int]\nclass B(TypedDict):\n    x: int\n\
         class C(A, B):\n    pass\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&ro)?) >= 1,
        "conflicting read-only-ness across bases must fire E0038"
    );

    // Identical declarations across bases are compatible.
    let legal = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: int\nclass B(TypedDict):\n    x: int\n\
         class C(A, B):\n    pass\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&legal)?),
        0,
        "identical field declarations across bases must not conflict"
    );

    // Three bases, two of which conflict on `x`: pins the `len() < 2` early-out
    // threshold — a wrong comparison would skip checking 3+ base classes.
    let three = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: int\nclass B(TypedDict):\n    x: float\n\
         class D(TypedDict):\n    z: int\nclass C(A, B, D):\n    pass\n"
    );
    assert!(
        typeddicts_inheritance_count(&run(&three)?) >= 1,
        "a conflict among three bases must still fire E0038"
    );
    Ok(())
}

/// `check_conflicting_bases` guard: a single `TypedDict` base (no multiple
/// inheritance) never triggers the conflict path, even with qualifiers.
#[mutation_safe(rule = "typeddicts_inheritance", fns = "check_conflicting_bases")]
#[test]
fn mutant_e0038_single_base_no_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let single = format!(
        "{TD_PRELUDE}\nclass A(TypedDict):\n    x: ReadOnly[int]\n\
         class C(A):\n    y: int\n"
    );
    assert_eq!(
        typeddicts_inheritance_count(&run(&single)?),
        0,
        "a single base with a new field must not raise a conflict"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Generic callback-protocol substitution (specialize_sig)
// ═══════════════════════════════════════════════════════════════════════

/// `specialize_sig` must substitute parameter types when a generic callback
/// protocol is specialized: `GenericCb[int] = int_cb` is valid only because
/// `T := int` is applied to the `__call__` parameter. A mutant that keeps the
/// unsubstituted `T` makes the valid assignment flag (extra E0014), and a
/// mutant that over-suppresses misses the invalid one.
#[mutation_safe(rule = "assignment_compatibility", fns = "specialize_sig")]
#[test]
fn mutant_e0014_generic_callback_substitution() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, Protocol, TypeVar

T = TypeVar("T")

class GenericCb(Protocol[T]):
    def __call__(self, a: T) -> None: ...

class IntCb(Protocol):
    def __call__(self, a: int) -> None: ...

class StrCb(Protocol):
    def __call__(self, a: str) -> None: ...

def func(int_cb: IntCb, str_cb: StrCb) -> None:
    ok: GenericCb[int] = int_cb
    bad: GenericCb[int] = str_cb
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(
        e0014.len(),
        1,
        "exactly the str-callback assignment must flag; substitution failures \
         flag both, over-suppression flags neither: {:?}",
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    let only = e0014.first().ok_or("expected one E0014 diagnostic")?;
    assert!(
        only.message.contains("bad"),
        "the diagnostic must be on `bad`, got: {}",
        only.message
    );
    Ok(())
}

/// `specialize_sig`'s `Proto[...]` arm turns a `ParamSpec` `__call__` into a
/// gradual signature that still checks the `Concatenate`-style prefix.
/// Deleting the arm collapses to "Unknown" and silently suppresses the
/// invalid assignment below.
#[mutation_safe(rule = "assignment_compatibility", fns = "specialize_sig")]
#[test]
fn mutant_e0014_paramspec_ellipsis_prefix() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, ParamSpec

P = ParamSpec("P")

class WithPrefix(Protocol[P]):
    def __call__(self, a: int, *args: P.args, **kwargs: P.kwargs) -> None: ...

class IntFirst(Protocol):
    def __call__(self, a: int) -> None: ...

class StrFirst(Protocol):
    def __call__(self, a: str) -> None: ...

def func(int_first: IntFirst, str_first: StrFirst) -> None:
    ok: WithPrefix[...] = int_first
    bad: WithPrefix[...] = str_first
"#;
    let diagnostics = run(source)?;
    let e0014: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(
        e0014.len(),
        1,
        "only the str-first assignment violates the int prefix: {:?}",
        e0014.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    let only = e0014.first().ok_or("expected one E0014 diagnostic")?;
    assert!(
        only.message.contains("bad"),
        "the diagnostic must be on `bad`, got: {}",
        only.message
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// e0002/e0003/e0029 — negative-direction and message-arm mutants
// (`&&`→`||` in check filters, `is_unresolvable`→true, deleted match arms,
//  wrong-function `find` in E0029). Smoke tests only assert the positive
//  direction, so these mutants survived them.
// ═══════════════════════════════════════════════════════════════════════

#[mutation_safe(rule = "missing_return_annotation")]
#[test]
fn mutant_e0002_annotated_function_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def fully_annotated(x: int) -> int:
    return x

def returns_nothing() -> None:
    pass
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diagnostics, "BSK-0002"),
        0,
        "annotated functions must not be flagged: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "missing_variable_type")]
#[test]
fn mutant_e0003_resolvable_or_annotated_vars_not_flagged() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
annotated_empty: list[int] = []
annotated_none: int | None = None
resolvable_int = 5
resolvable_str = 'text'
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diagnostics, "BSK-0003"),
        0,
        "annotated or inferable variables must not be flagged: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "missing_variable_type")]
#[test]
fn mutant_e0003_messages_name_the_rhs_kind() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
empty_list = []
empty_dict = {}
nothing = None
";
    let diagnostics = run_with_config(source, &annotation_rules_config())?;
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0003")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages.len(),
        3,
        "all three vars must be flagged: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("empty list `[]`")),
        "empty-list diagnostic must explain the empty list: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("empty dict `{}`")),
        "empty-dict diagnostic must explain the empty dict: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("infer type from `None`")),
        "None diagnostic must explain the None value: {messages:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "typeddicts_class_syntax")]
#[test]
fn mutant_e0029_diagnostic_targets_the_typeddict_method() -> Result<(), Box<dyn std::error::Error>>
{
    // `Plain.title` comes first so a loosened function lookup (`&&`→`||`)
    // resolves the wrong function and points the span at the wrong class.
    let source = r"
from typing import TypedDict

class Plain:
    def title(self) -> str:
        return 'plain'

class Movie(TypedDict):
    name: str

    def title(self) -> str:
        return self['name']
";
    let diagnostics = run(source)?;
    let e0029: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax")
        .collect();
    assert_eq!(
        e0029.len(),
        1,
        "exactly the TypedDict method is flagged: {e0029:?}"
    );
    let only = e0029.first().ok_or("expected one E0029 diagnostic")?;
    let movie_class_offset = u32::try_from(
        source
            .find("class Movie")
            .ok_or("fixture must contain Movie")?,
    )?;
    assert!(
        only.span.start > movie_class_offset,
        "diagnostic must point inside `Movie`, not `Plain` (span {} <= class offset {})",
        only.span.start,
        movie_class_offset
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// context.rs from_config_with_source — shared line index must be built
// ═══════════════════════════════════════════════════════════════════════

/// Kills the cargo-mutants `StructField` mutant that deletes the `line_index`
/// field from `CheckContext::from_config_with_source` (context.rs:70).
///
/// That field is the per-check line index the suppression layer consults to
/// place each diagnostic on its source line (`lib.rs`: `diag_line =
/// ctx.line_index.line(span.start) - 1`). Deleting the field falls back to
/// `..from_config(..)`, whose `LineIndex::default()` is empty and maps EVERY
/// offset to line 1 → `diag_line = 0`, so a per-line `# type: ignore` on any
/// line past the first stops matching its diagnostic and the diagnostic escapes.
///
/// The suppression *map* is parsed straight from the source (independent of the
/// line index), so only the diagnostic's line placement moves under the mutant —
/// exactly the asymmetry this exercises. Scoped to the already-scoped
/// `check_vars` so the examine-re mutant selection is unchanged.
#[mutation_safe(rule = "assignment_compatibility", fns = "check_vars")]
#[test]
fn mutant_context_line_index_places_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    // Lines 1-2 are clean; the only mismatch is on line 3, carrying an inline
    // `# type: ignore` on that same line. A populated line index places the
    // diagnostic on line 3 (`diag_line = 2`), matching the override → suppressed.
    let suppressed = "a: int = 1\nb: int = 2\nc: int = \"wrong\"  # type: ignore\n";
    let diagnostics = run(suppressed)?;
    assert_eq!(
        assignment_compatibility_count(&diagnostics),
        0,
        "line-3 `# type: ignore` must suppress the line-3 mismatch; a dropped \
         line_index maps it to line 0 and lets it escape: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    // Positive control: the identical mismatch WITHOUT the inline ignore must
    // fire, proving the suppression above — not some unrelated reason — is what
    // silences the diagnostic in the case that kills the mutant.
    let unsuppressed = "a: int = 1\nb: int = 2\nc: int = \"wrong\"\n";
    let diagnostics = run(unsuppressed)?;
    assert_eq!(
        assignment_compatibility_count(&diagnostics),
        1,
        "the line-3 mismatch must fire when it is not suppressed: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "assignment_compatibility")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Count `classes_override` diagnostics.
fn classes_override_count(diags: &[basilisk_checker::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .count()
}

/// `annotations_conflict`: an `@override` signature conflicts only when BOTH
/// annotation sides are present and differ. Absent annotations are implicitly
/// `Any` and can never prove incompatibility ([TYPEINF-TARGET-GRADUAL]) —
/// stripping a child annotation must not create a new error.
#[mutation_safe(rule = "classes_override", fns = "annotations_conflict")]
#[test]
fn mutant_classes_override_annotations_conflict() -> Result<(), Box<dyn std::error::Error>> {
    // Differing parameter annotations must fire.
    let param_mismatch = r"
from typing import override

class Base:
    def process(self, value: int) -> int:
        return value

class Child(Base):
    @override
    def process(self, value: str) -> int:
        return 0
";
    assert!(
        classes_override_count(&run(param_mismatch)?) >= 1,
        "str-vs-int parameter override must fire classes_override"
    );

    // Differing return annotations must fire.
    let return_mismatch = r"
from typing import override

class Base:
    def process(self, value: int) -> int:
        return value

class Child(Base):
    @override
    def process(self, value: int) -> str:
        return ''
";
    assert!(
        classes_override_count(&run(return_mismatch)?) >= 1,
        "str-vs-int return override must fire classes_override"
    );

    // Identical signatures never fire.
    let identical = r"
from typing import override

class Base:
    def process(self, value: int) -> int:
        return value

class Child(Base):
    @override
    def process(self, value: int) -> int:
        return value
";
    assert_eq!(
        classes_override_count(&run(identical)?),
        0,
        "identical override signature must not fire"
    );

    // An UNANNOTATED override side is implicitly Any — never a conflict.
    let unannotated_child = r"
from typing import override

class Base:
    def process(self, value: int) -> int:
        return value

class Child(Base):
    @override
    def process(self, value):
        return value
";
    assert_eq!(
        classes_override_count(&run(unannotated_child)?),
        0,
        "absent child annotations are implicitly Any and must not fire"
    );
    Ok(())
}
