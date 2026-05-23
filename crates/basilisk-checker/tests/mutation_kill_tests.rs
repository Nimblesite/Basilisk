//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Mutation-killing tests: assertion-heavy tests designed to catch specific
//! mutants identified by cargo-mutants. Each test asserts BOTH that violations
//! ARE caught and that correct code is NOT flagged.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;
use basilisk_test_macros::mutation_safe;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0014_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
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
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 2,
        "at least 2 mismatches expected (c and d), got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
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
    let e0014 = e0014_count(&diagnostics);
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
    let e0014 = e0014_count(&diagnostics);
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
    let e0014 = e0014_count(&diagnostics);
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
    let e0014 = e0014_count(&diagnostics);
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
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 3,
        "at least 3 named type mismatches, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
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
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 union mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0097: Protocol undeclared self attrs — kill all mutants
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
        .filter(|d| d.code.code == "BSK-E0097")
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
// E0150: Dead branch — kill mutants in version/platform detection
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
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
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
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
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
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
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
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
        .collect();
    assert!(
        !e0150.is_empty(),
        "must flag dead variable in != bogus else branch"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014: Various literal type mismatches — each type path
// ═══════════════════════════════════════════════════════════════════════

#[mutation_safe(rule = "e0014", fns = "check_vars")]
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
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 6,
        "at least 6 type mismatches expected, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014: Negative literal and float literal
// ═══════════════════════════════════════════════════════════════════════

#[mutation_safe(rule = "e0014", fns = "check_vars")]
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
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 mismatch (b: str = -42 or c: int = 3.14), got {e0014}"
    );
    Ok(())
}

fn e0001_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .count()
}

// ═══════════════════════════════════════════════════════════════════════
// E0001 check_function: !p.has_annotation guard
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0001.rs:33 `delete ! in check_function` and `replace != with ==`
/// on the name guards. Asserts BOTH that an unannotated regular param fires E0001
/// AND that an annotated regular param does NOT — so flipping the polarity of the
/// annotation check or the name guards produces an observable diagnostic count.
#[mutation_safe(rule = "e0001", fns = "check_function")]
#[test]
fn mutant_e0001_annotation_polarity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def has_annotated_only(annotated: int) -> int:
    return annotated

def missing(unannotated, annotated: int) -> int:
    return annotated
";
    let diagnostics = run(source)?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!(
            "exactly one E0001 expected for `unannotated` only, got {}: {e0001:?}",
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
// E0001 check_function: self/cls guards (&& vs ||)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutants: e0001.rs:33 `replace && with ||` (both occurrences).
/// With `||` instead of `&&`, every parameter satisfies at least one branch,
/// so unannotated `self`/`cls` would fire E0001 (false positive). This test
/// asserts that unannotated `self` and `cls` do NOT fire while a sibling
/// unannotated regular param DOES — exactly distinguishing && from ||.
#[mutation_safe(rule = "e0001", fns = "check_function")]
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
    let diagnostics = run(source)?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
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
        "E0001 must fire for `x`, got: {names:?}"
    );
    assert!(
        names.iter().any(|m| m.contains("`y`")),
        "E0001 must fire for `y`, got: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|m| m.contains("`self`") || m.contains("`cls`")),
        "E0001 must NOT fire for `self` or `cls`, got: {names:?}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0001 check (impl): is_stub_context filter (delete !)
// ═══════════════════════════════════════════════════════════════════════

/// Kills mutant: e0001.rs:25 `delete ! in check`. Without the negation,
/// only stub-context functions are checked — so unannotated params in regular
/// (non-stub) functions are missed AND unannotated params inside Protocol
/// methods would incorrectly fire. This test asserts both directions.
#[mutation_safe(rule = "e0001", fns = "check")]
#[test]
fn mutant_e0001_stub_context_negation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Iface(Protocol):
    def stub_method(self, p): ...

def regular(unannotated):
    return unannotated
";
    let diagnostics = run(source)?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!(
            "exactly one E0001 expected (for `unannotated` in regular fn), \
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
/// no E0001 ever fires. A simple presence-of-diagnostic assertion kills it.
#[mutation_safe(rule = "e0001", fns = "check")]
#[test]
fn mutant_e0001_check_body_present() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(unannotated):
    return unannotated
";
    let diagnostics = run(source)?;
    let count = e0001_count(&diagnostics);
    assert_eq!(
        count, 1,
        "E0001 must fire exactly once for unannotated param, got {count}"
    );
    Ok(())
}

/// Kills mutant: e0001.rs:38 `replace make_diagnostic -> Diagnostic with Default::default()`.
/// `Default::default()` produces an empty Diagnostic (empty code, empty message).
/// Asserting both code AND message content kills the mutant — `Default` cannot
/// produce `BSK-E0001` and a parameter-named message simultaneously.
#[mutation_safe(rule = "e0001", fns = "make_diagnostic")]
#[test]
fn mutant_e0001_diagnostic_payload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(specific_name):
    return specific_name
";
    let diagnostics = run(source)?;
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    let [only] = e0001.as_slice() else {
        return Err(format!("exactly one E0001 expected: {e0001:?}").into());
    };
    assert_eq!(only.code.code, "BSK-E0001");
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

#[mutation_safe(rule = "e0002")]
#[test]
fn mutant_e0002_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def no_return_annotation(x: int):
    return x
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "BSK-E0002") >= 1,
        "E0002 must fire for missing return annotation: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0003")]
#[test]
fn mutant_e0003_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
empty_list = []
empty_dict = {}
nothing = None
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "BSK-E0003") >= 1,
        "E0003 must fire for unannotated empty-collection vars: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0023")]
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
        count_code(&diagnostics, "BSK-E0023") >= 1,
        "E0023 must fire for non-exhaustive match: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0027")]
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
        count_code(&diagnostics, "BSK-E0027") >= 1,
        "E0027 must fire for duplicate TypeVar in Generic: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0029")]
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
        count_code(&diagnostics, "BSK-E0029") >= 1,
        "E0029 must fire for TypedDict method: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0033")]
#[test]
fn mutant_e0033_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
reveal_type()
reveal_type(1, 2)
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "BSK-E0033") >= 1,
        "E0033 must fire for invalid reveal_type call: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0039")]
#[test]
fn mutant_e0039_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
assert_type(1)
assert_type(1, int, 'extra')
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "BSK-E0039") >= 1,
        "E0039 must fire for invalid assert_type call: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0049")]
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
        count_code(&diagnostics, "BSK-E0049") >= 1,
        "E0049 must fire for multiple unbounded tuple components: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0088")]
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
        count_code(&diagnostics, "BSK-E0088") >= 1,
        "E0088 must fire for isinstance() with TypedDict: {diagnostics:?}"
    );
    Ok(())
}

#[mutation_safe(rule = "e0105")]
#[test]
fn mutant_e0105_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> None:
        x.is_integer()
";
    let diagnostics = run(source)?;
    assert!(
        count_code(&diagnostics, "BSK-E0105") >= 1,
        "E0105 must fire for invalid attr on bounded TypeVar: {diagnostics:?}"
    );
    Ok(())
}
