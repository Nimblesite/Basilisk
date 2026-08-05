//! Tests for [LINESCANPLAN-AST-MIGRATION] / [CHKARCH-TESTING]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! Assertion-dense structural tests for the shared type-expression judge and
//! the four rules rebuilt on it (issues #379, #408–#412). Every test here
//! asserts EXACT flagged/unflagged name sets — never "ran without crashing".
//!
//! The permutation sections mirror the AST-preserving mutations that exposed
//! the old text scanners (consistent import renames, whitespace reformatting):
//! a structural checker's verdicts must be IDENTICAL under both.
//!
//! Tests prefixed `red_pin_` document REAL, currently-missing capability and
//! are EXPECTED TO FAIL until the gap is closed. Per the repo's testing
//! rules they must never be deleted or weakened — they are the ratchet for
//! honest completion of the migration.

use std::collections::BTreeSet;

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Names extracted from a rule's diagnostics via the `... for `NAME`` suffix.
fn flagged_names(diags: &[basilisk_checker::Diagnostic], code: &str) -> BTreeSet<String> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .filter_map(|d| d.message.split('`').nth(3).map(str::to_owned))
        .collect()
}

fn codes<'d>(diags: &'d [basilisk_checker::Diagnostic]) -> Vec<&'d str> {
    diags.iter().map(|d| d.code.code).collect()
}

fn count_code(diags: &[basilisk_checker::Diagnostic], code: &str) -> usize {
    diags.iter().filter(|d| d.code.code == code).count()
}

// ═══════════════════════════════════════════════════════════════════════
// Explicit TypeAlias — full bad/good battery under a RENAMED import.
// The old checker recovered aliases with match_indices("TypeAlias as ")
// and is_invalid_rhs() text scans; a consistent rename plus reformat broke
// both. Every verdict below must hold regardless of spelling.
// ═══════════════════════════════════════════════════════════════════════

const EXPLICIT_BATTERY: &str = r#"
from typing import TypeAlias as AuditTypeAlias
var1 = 3
BadEval: AuditTypeAlias = eval("int")
BadList: AuditTypeAlias = [int, str]
BadTuple: AuditTypeAlias = ((int, str),)
BadComp: AuditTypeAlias = [int for i in range(1)]
BadDict: AuditTypeAlias = {"a": "b"}
BadLambdaCall: AuditTypeAlias = (lambda: int)()
BadIndex: AuditTypeAlias = [int][0]
BadTernary: AuditTypeAlias = int if 1 < 3 else str
BadVarRef: AuditTypeAlias = var1
BadBool: AuditTypeAlias = True
BadInt: AuditTypeAlias = 1
BadBoolOp: AuditTypeAlias = list or set
BadFString: AuditTypeAlias = f"{'int'}"
GoodUnion: AuditTypeAlias = int | str
GoodSubscript: AuditTypeAlias = list[int]
GoodForwardRef: AuditTypeAlias = "int | str"
GoodNestedRef: AuditTypeAlias = list["int | str"]
GoodNone: AuditTypeAlias = None
"#;

#[test]
fn explicit_alias_battery_under_renamed_import() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run(EXPLICIT_BATTERY)?;
    let flagged = flagged_names(&diags, "aliases_implicit");
    let expected_bad = [
        "BadEval",
        "BadList",
        "BadTuple",
        "BadComp",
        "BadDict",
        "BadLambdaCall",
        "BadIndex",
        "BadTernary",
        "BadVarRef",
        "BadBool",
        "BadInt",
        "BadBoolOp",
        "BadFString",
    ];
    for name in expected_bad {
        assert!(flagged.contains(name), "`{name}` must be flagged; got {flagged:?}");
    }
    for name in ["GoodUnion", "GoodSubscript", "GoodForwardRef", "GoodNestedRef", "GoodNone"] {
        assert!(!flagged.contains(name), "`{name}` must NOT be flagged; got {flagged:?}");
    }
    assert_eq!(
        flagged.len(),
        expected_bad.len(),
        "exactly the 13 bad aliases flag, nothing else: {flagged:?}"
    );
    Ok(())
}

#[test]
fn explicit_alias_battery_whitespace_mutated() -> Result<(), Box<dyn std::error::Error>> {
    // The same verdicts with mutated spacing everywhere the old scanners
    // keyed on it: no space around `|`, doubled spaces around keywords,
    // spaces inside brackets and calls.
    let source = r#"
from typing import TypeAlias as AuditTypeAlias
var1 = 3
BadTernary: AuditTypeAlias = int  if  1 < 3  else  str
BadBoolOp: AuditTypeAlias = list  or  set
BadAndOp: AuditTypeAlias = list  and  set
BadLambdaCall: AuditTypeAlias = ( lambda : int ) ( )
BadTuple: AuditTypeAlias = ( ( int , str ) , )
BadEval: AuditTypeAlias = eval ( "int" )
GoodUnion: AuditTypeAlias = int|str
GoodSpacedUnion: AuditTypeAlias = int   |   str
GoodSubscript: AuditTypeAlias = dict[ str , int ]
"#;
    let flagged = flagged_names(&run(source)?, "aliases_implicit");
    for name in ["BadTernary", "BadBoolOp", "BadAndOp", "BadLambdaCall", "BadTuple", "BadEval"] {
        assert!(flagged.contains(name), "`{name}` must survive reformatting; got {flagged:?}");
    }
    for name in ["GoodUnion", "GoodSpacedUnion", "GoodSubscript"] {
        assert!(!flagged.contains(name), "`{name}` must NOT be flagged; got {flagged:?}");
    }
    Ok(())
}

#[test]
fn explicit_alias_dotted_and_aliased_module_spellings() -> Result<(), Box<dyn std::error::Error>> {
    for (header, annotation) in [
        ("import typing", "typing.TypeAlias"),
        ("import typing as t", "t.TypeAlias"),
        ("from typing_extensions import TypeAlias as TEA", "TEA"),
    ] {
        let source =
            format!("{header}\nBad: {annotation} = [int, str]\nGood: {annotation} = int | str\n");
        let flagged = flagged_names(&run(&source)?, "aliases_implicit");
        assert!(
            flagged.contains("Bad"),
            "`Bad` must be flagged under `{annotation}`; got {flagged:?}"
        );
        assert!(
            !flagged.contains("Good"),
            "`Good` must NOT be flagged under `{annotation}`; got {flagged:?}"
        );
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// PEP 695 `type` statements — structural verdicts (#379).
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn type_statement_attribute_on_subscript_fires() -> Result<(), Box<dyn std::error::Error>> {
    // #379: attribute access on a subscript is a value operation, not a
    // type. The old code accepted every Attribute node unconditionally.
    let diags = run("type Bad = list[int].attr\n")?;
    assert_eq!(count_code(&diags, "aliases_type_statement"), 1, "{:?}", codes(&diags));
    Ok(())
}

#[test]
fn type_statement_dotted_names_and_identifier_substrings_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "
import collections.abc
class Blambda: pass
class Morchid: pass
type Dotted = collections.abc.Sequence
type Sub = Blambda
type Deep = dict[str, Morchid]
";
    let diags = run(source)?;
    assert_eq!(
        count_code(&diags, "aliases_type_statement"),
        0,
        "identifiers containing keyword substrings are not verdict inputs: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn type_statement_forward_ref_contents_are_judged() -> Result<(), Box<dyn std::error::Error>> {
    // Lazily-evaluated statements accept strings anywhere, but content must
    // itself be a type expression.
    let good = run("type Recursive = \"Recursive\" | int\ntype Spaced = \"int   |   str\"\n")?;
    assert_eq!(count_code(&good, "aliases_type_statement"), 0, "{:?}", codes(&good));
    let bad = run("type Bad = \"[int, str]\"\n")?;
    assert_eq!(count_code(&bad, "aliases_type_statement"), 1, "{:?}", codes(&bad));
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Annotation validity (annotations_forward_refs) — quoted and unquoted.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn forward_ref_string_battery() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import types
var1 = 1
def f(
    p1: "eval('int')",
    p2: "[int, str]",
    p3: "( int , str )",
    p4: "{}",
    p5: "int  if  1 < 3  else  str",
    p6: "var1",
    p7: "types",
    p8: "int  or  str",
    p9: "-1",
    p10: "int | str",
    p11: "list[int]",
) -> None: ...
"#;
    let flagged: BTreeSet<String> = run(source)?
        .iter()
        .filter(|d| d.code.code == "annotations_forward_refs")
        .filter_map(|d| d.message.split('`').nth(1).map(str::to_owned))
        .collect();
    for name in ["p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9"] {
        assert!(flagged.contains(name), "`{name}` must be flagged; got {flagged:?}");
    }
    for name in ["p10", "p11"] {
        assert!(!flagged.contains(name), "`{name}` must NOT be flagged; got {flagged:?}");
    }
    Ok(())
}

#[test]
fn string_operand_in_union_is_runtime_error() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("bad3: \"ClassA\" | int\nbad4: int | \"ClassA\"\nclass ClassA: ...\n")?;
    assert_eq!(count_code(&diags, "annotations_forward_refs"), 2, "{:?}", codes(&diags));
    Ok(())
}

#[test]
fn triple_quoted_multiline_forward_ref_is_valid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("value: \"\"\"\n    int |\n    str |\n    list[int]\n\"\"\"\n")?;
    assert_eq!(count_code(&diags, "annotations_forward_refs"), 0, "{:?}", codes(&diags));
    Ok(())
}

#[test]
fn pep646_star_unpack_annotations_are_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = "
from typing import TypeVarTuple
Ts = TypeVarTuple('Ts')
def f(*args: *Ts) -> None: ...
def g(*args: *tuple[int, ...]) -> None: ...
";
    let diags = run(source)?;
    assert_eq!(count_code(&diags, "annotations_forward_refs"), 0, "{:?}", codes(&diags));
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Annotated[...] first argument (qualifiers_annotated).
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn annotated_battery_with_spacing_mutations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated, Callable, TypeAlias
Bad1: Annotated[ [int, str] , "" ]
Bad2: Annotated[( ( int , str ) , ), ""]
Bad3: Annotated[int  if  1 < 3  else  str, ""]
Bad4: Annotated[list  or  set, ""]
Bad5: Annotated[undefined_name_xyz, ""]
TooFew: Annotated[int]
Good1: Annotated[Callable[ [int] , str ], "meta"]
Good2: Annotated["int | str", ""]
Good3: Annotated[int|str, 3, "", max(1, 2)]
"#;
    let flagged = flagged_names(&run(source)?, "qualifiers_annotated");
    for name in ["Bad1", "Bad2", "Bad3", "Bad4", "Bad5"] {
        assert!(flagged.contains(name), "`{name}` must be flagged; got {flagged:?}");
    }
    for name in ["Good1", "Good2", "Good3"] {
        assert!(!flagged.contains(name), "`{name}` must NOT be flagged; got {flagged:?}");
    }
    Ok(())
}

#[test]
fn annotated_alias_calls_fire_but_class_alias_calls_do_not() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated, TypeAlias
SmallInt: TypeAlias = Annotated[int, ""]
ListAlias: TypeAlias = list
bad = SmallInt(1)
ok = ListAlias()
"#;
    let diags = run(source)?;
    let messages: Vec<&str> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_annotated")
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("SmallInt")),
        "calling an Annotated alias must be flagged: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("ListAlias")),
        "an alias to a constructible class is callable: {messages:?}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Parameterization: honest arity, ParamSpec position, and bounds.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn variadic_alias_has_no_upper_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = "
from typing import TypeAlias, TypeVarTuple, Unpack
Ts = TypeVarTuple('Ts')
IntTuple: TypeAlias = tuple[int, Unpack[Ts]]
a: IntTuple[int]
b: IntTuple[int, str, bytes, float]
";
    let diags = run(source)?;
    assert_eq!(
        count_code(&diags, "aliases_implicit"),
        0,
        "a TypeVarTuple alias accepts any arity: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn paramspec_argument_position_is_checked_positionally() -> Result<(), Box<dyn std::error::Error>> {
    // #409: the ParamSpec argument is located by the parameter's position in
    // the alias, not guessed from "all args look simple".
    let source = "
from typing import Callable, Concatenate, ParamSpec, TypeAlias, TypeVar
P = ParamSpec('P')
R = TypeVar('R')
Alias: TypeAlias = Callable[Concatenate[int, P], R]
def bad(x: Alias[int, int]) -> None: ...
def good(x: Alias[[str, str], None]) -> None: ...
def also_good(x: Alias[..., None]) -> None: ...
";
    let diags = run(source)?;
    let paramspec_errors: Vec<&str> = diags
        .iter()
        .filter(|d| d.code.code == "aliases_implicit" && d.message.contains("ParamSpec"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        paramspec_errors.len(),
        1,
        "exactly the bad ParamSpec argument flags: {paramspec_errors:?}"
    );
    Ok(())
}

#[test]
fn typevar_bounds_checked_through_local_class_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    // #410: the bound check goes through the subtyping context — module-local
    // inheritance works, and str-vs-float uses the numeric tower. Unknown
    // names abstain instead of erroring.
    let source = "
from typing import TypeAlias, TypeVar
from external_module import Mystery
class Base: ...
class Sub(Base): ...
class Other: ...
TB = TypeVar('TB', bound=Base)
Boxed: TypeAlias = list[TB]
ok: Boxed[Sub]
bad: Boxed[Other]
unknown: Boxed[Mystery]
TF = TypeVar('TF', bound=float)
Nums: TypeAlias = list[TF]
ok2: Nums[bool]
bad2: Nums[str]
";
    let diags = run(source)?;
    let bound_errors: Vec<&str> = diags
        .iter()
        .filter(|d| d.code.code == "aliases_implicit" && d.message.contains("does not satisfy"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(bound_errors.len(), 2, "exactly Other and str violate: {bound_errors:?}");
    assert!(bound_errors.iter().any(|m| m.contains("`Other`")), "{bound_errors:?}");
    assert!(bound_errors.iter().any(|m| m.contains("`str`")), "{bound_errors:?}");
    Ok(())
}

#[test]
fn implicit_alias_detection_ignores_name_case() -> Result<(), Box<dyn std::error::Error>> {
    // #411: alias-hood comes from the RHS structure. A lowercase alias to a
    // union still cannot be instantiated; an uppercase runtime value is
    // still not a type.
    let source = "
lowercase_alias = int | str
x = lowercase_alias()
Uppercase_value = [1, 2, 3]
def f(p: Uppercase_value) -> None: ...
";
    let diags = run(source)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "aliases_implicit"
                && d.message.contains("Cannot instantiate union type alias `lowercase_alias`")),
        "union alias instantiation must not depend on name casing: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "aliases_implicit" && d.message.contains("`Uppercase_value`")),
        "runtime value in annotation must not depend on name casing: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// RED PINS — real gaps, expected to FAIL until closed. Do not delete,
// weaken, or skip ([CHKARCH-TESTING]); closing the gap turns them green.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn red_pin_assert_type_on_union_of_tuples_must_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // tuples_type_compat gap: a checker that does not narrow by len() must
    // still report that `val` is not `tuple[int]`. Today both resolver and
    // oracle abstain, and the file fails conformance. Previously this line
    // was "passed" by a bogus text-scan diagnostic from qualifiers_annotated.
    let source = "
from typing import TypeAlias, assert_type
Func5Input: TypeAlias = tuple[int] | tuple[str, str]
def func5(val: Func5Input):
    if len(val) == 1:
        assert_type(val, tuple[int])
";
    let diags = run(source)?;
    assert!(
        diags.iter().any(|d| d.code.code.starts_with("directives_assert_type")),
        "assert_type(val, tuple[int]) with val: tuple-union must mismatch \
         (or the checker must narrow); got {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn red_pin_annotated_aliased_import_is_recognised() -> Result<(), Box<dyn std::error::Error>> {
    // `from typing import Annotated as Ann` — the rule recognises the
    // subscript base by resolved import, so the aliased spelling must flag
    // an invalid first argument exactly like the plain spelling.
    let source = "
from typing import Annotated as Ann
Bad1: Ann[[int, str], \"\"]
";
    let diags = run(source)?;
    assert_eq!(
        count_code(&diags, "qualifiers_annotated"),
        1,
        "aliased Annotated import must be recognised: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn red_pin_runtime_name_in_module_var_annotation() -> Result<(), Box<dyn std::error::Error>> {
    // Runtime-variable names are invalid in EVERY annotation position, not
    // only function parameters. Module-var coverage is still missing.
    let source = "
bad_value = [1, 2, 3]
x: bad_value = None
";
    let diags = run(source)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "aliases_implicit" || d.code.code == "annotations_forward_refs"),
        "a runtime variable used as a module-var annotation must be flagged; got {:?}",
        codes(&diags)
    );
    Ok(())
}
