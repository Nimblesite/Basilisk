//! Integration tests for basilisk-checker.

use basilisk_checker::{check, Severity};
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn no_diagnostics_for_fully_annotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def greet(name: str) -> str:\n    return name\n")?;
    assert!(
        diags.is_empty(),
        "fully annotated function should produce no diagnostics"
    );
    Ok(())
}

#[test]
fn emits_e0001_for_missing_parameter_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data) -> None:\n    pass\n")?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-E0001");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_e0002_for_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data: str):\n    pass\n")?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-E0002");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_both_for_unannotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data):\n    pass\n")?;
    assert_eq!(diags.len(), 2, "should emit E0001 and E0002");

    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(codes.contains(&"BSK-E0001"));
    assert!(codes.contains(&"BSK-E0002"));
    Ok(())
}

#[test]
fn emits_one_e0001_per_unannotated_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def multi(a, b, c) -> None:\n    pass\n")?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert_eq!(
        count, 3,
        "three unannotated params should produce three E0001s"
    );
    Ok(())
}

#[test]
fn handles_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("")?;
    assert!(diags.is_empty());
    Ok(())
}

#[test]
fn all_diagnostics_have_nonempty_message() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def bad(x):\n    pass\n")?;
    for d in &diags {
        assert!(!d.message.is_empty());
    }
    Ok(())
}

#[test]
fn all_diagnostics_have_docs_url() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def bad(x):\n    pass\n")?;
    for d in &diags {
        assert!(d.code.docs_url.starts_with("https://"));
    }
    Ok(())
}

#[test]
fn severity_error_displays_as_error() {
    assert_eq!(format!("{}", Severity::Error), "error");
}

#[test]
fn severity_warning_displays_as_warning() {
    assert_eq!(format!("{}", Severity::Warning), "warning");
}

#[test]
fn severity_error_greater_than_warning() {
    assert!(Severity::Error > Severity::Warning);
}

// ---------------------------------------------------------------------------
// E0003 — Missing variable type: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0003_annotated_empty_list_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Annotated variable: E0003 must NOT fire (has_annotation = true)
    let diags = run("items: list[int] = []\n")?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0003").collect();
    assert!(e3.is_empty(), "annotated empty-list variable must not trigger E0003");
    Ok(())
}

#[test]
fn e0003_unannotated_str_literal_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Unannotated but with an inferrable literal — E0003 must NOT fire
    let diags = run("name = \"hello\"\n")?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0003").collect();
    assert!(e3.is_empty(), "unannotated str literal must not trigger E0003");
    Ok(())
}

#[test]
fn e0003_fires_for_all_three_unresolvable_rhs_kinds() -> Result<(), Box<dyn std::error::Error>> {
    // Covers EmptyList, EmptyDict, and NoneValue branches in make_diagnostic
    let diags = run("a = []\nb = {}\nc = None\n")?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0003").collect();
    assert_eq!(e3.len(), 3, "all three unresolvable kinds must fire E0003");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0012 — Argument type mismatch: all match arms
// ---------------------------------------------------------------------------

#[test]
fn e0012_bool_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bool) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "bool param + str literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_float_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: float) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "float param + str literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_bytes_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bytes) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "bytes param + str literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_int_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "int param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_str_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "str param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_float_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: float) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "float param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_int_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "int param + float literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_str_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "str param + float literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_bool_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bool) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "bool param + float literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_str_param_receives_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "str param + int literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_bytes_param_receives_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bytes) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert_eq!(e12.len(), 1, "bytes param + int literal must fire E0012");
    Ok(())
}

#[test]
fn e0012_compatible_int_arg_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // int param + int literal: compatible → no E0012
    let src = "def foo(x: int) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert!(e12.is_empty(), "compatible int arg must not fire E0012");
    Ok(())
}

#[test]
fn e0012_unknown_callee_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Callee not defined in same module — no diagnostic
    let src = "unknown_func(42, \"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert!(e12.is_empty(), "call to unknown function must not fire E0012");
    Ok(())
}

#[test]
fn e0012_extra_args_beyond_params_does_not_crash() -> Result<(), Box<dyn std::error::Error>> {
    // More positional args than declared params: checker must handle gracefully (break path)
    let src = "def foo(x: int) -> None: pass\nfoo(1, \"extra\", b\"more\")\n";
    let diags = run(src)?;
    // Only the first arg is checked (x: int, arg=1) — no E0012
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert!(e12.is_empty(), "extra args beyond params must not fire E0012 for out-of-range args");
    Ok(())
}

#[test]
fn e0012_unannotated_param_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Param has no annotation: E0012 must NOT fire
    let src = "def foo(x) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0012").collect();
    assert!(e12.is_empty(), "unannotated param must not fire E0012");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0014 — Assignment type mismatch: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0014_bool_annotation_with_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("flag: bool = \"yes\"\n")?;
    let e14: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0014").collect();
    assert_eq!(e14.len(), 1, "bool: str mismatch must fire E0014");
    Ok(())
}

#[test]
fn e0014_float_annotation_with_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("ratio: float = \"1.5\"\n")?;
    let e14: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0014").collect();
    assert_eq!(e14.len(), 1, "float: str mismatch must fire E0014");
    Ok(())
}

#[test]
fn e0014_compatible_annotation_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // int annotation with int literal — compatible
    let diags = run("count: int = 42\n")?;
    let e14: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0014").collect();
    assert!(e14.is_empty(), "compatible int=int must not fire E0014");
    Ok(())
}

#[test]
fn e0014_annotation_at_end_of_file_no_newline() -> Result<(), Box<dyn std::error::Error>> {
    // Line without trailing newline — extract_annotation uses source.len() as line_end
    let diags = run("x: int = \"str\"")?;
    let e14: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0014").collect();
    assert_eq!(e14.len(), 1, "annotation at end of file (no trailing newline) must still fire");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0015 — Invalid type arg count: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0015_frozenset_with_two_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: frozenset[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert_eq!(e15.len(), 1, "frozenset[int, str] must fire E0015");
    Ok(())
}

#[test]
fn e0015_set_with_two_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: set[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert_eq!(e15.len(), 1, "set[int, str] must fire E0015");
    Ok(())
}

#[test]
fn e0015_dict_with_one_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: dict[str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert_eq!(e15.len(), 1, "dict[str] (one arg) must fire E0015");
    Ok(())
}

#[test]
fn e0015_correct_list_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: list[int]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert!(e15.is_empty(), "correct list[int] must not fire E0015");
    Ok(())
}

#[test]
fn e0015_vararg_with_invalid_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(*args: list[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert_eq!(e15.len(), 1, "vararg with list[int, str] must fire E0015");
    Ok(())
}

#[test]
fn e0015_kwarg_with_invalid_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(**kwargs: dict[str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert_eq!(e15.len(), 1, "kwarg with dict[str] must fire E0015");
    Ok(())
}

#[test]
fn e0015_nested_generic_correct_count() -> Result<(), Box<dyn std::error::Error>> {
    // dict[list[int], str] has 2 top-level args — must NOT fire
    let src = "def foo(x: dict[list[int], str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0015").collect();
    assert!(e15.is_empty(), "dict[list[int], str] has correct arg count");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0016 — Incompatible override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0016_different_param_count_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> None: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0016").collect();
    assert_eq!(e16.len(), 1, "different param count must fire E0016");
    Ok(())
}

#[test]
fn e0016_only_return_type_differs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base') -> int: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> str: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0016").collect();
    assert_eq!(e16.len(), 1, "different return type must fire E0016");
    Ok(())
}

#[test]
fn e0016_compatible_override_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> int: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child', x: int) -> int: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0016").collect();
    assert!(e16.is_empty(), "compatible override must not fire E0016");
    Ok(())
}

#[test]
fn e0016_base_not_in_module_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Base class defined externally — cannot check, must not fire
    let src = concat!(
        "from typing import override\n",
        "class Child(SomeExternalBase):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0016").collect();
    assert!(e16.is_empty(), "external base must not fire E0016");
    Ok(())
}

#[test]
fn e0016_method_without_override_decorator_not_checked() -> Result<(), Box<dyn std::error::Error>>
{
    // Method overrides base but has no @override — E0016 must NOT fire (that's E0025)
    let src = concat!(
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> int: pass\n",
        "class Child(Base):\n",
        "    def method(self: 'Child') -> str: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0016").collect();
    assert!(e16.is_empty(), "method without @override must not fire E0016");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0017 — Incompatible variable override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0017_unannotated_child_attr_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Base:\n", "    count: int = 0\n", "class Child(Base):\n",
        "    count = 0\n",);
    let diags = run(src)?;
    let e17: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0017").collect();
    assert!(e17.is_empty(), "unannotated child attribute must not fire E0017");
    Ok(())
}

#[test]
fn e0017_base_attr_not_annotated_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count = 0\n",
        "class Child(Base):\n",
        "    count: str = \"zero\"\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0017").collect();
    assert!(e17.is_empty(), "unannotated base attribute must not fire E0017");
    Ok(())
}

#[test]
fn e0017_same_annotation_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count: int = 0\n",
        "class Child(Base):\n",
        "    count: int = 1\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0017").collect();
    assert!(e17.is_empty(), "same annotation must not fire E0017");
    Ok(())
}

#[test]
fn e0017_attr_only_in_child_not_in_base_does_not_fire() -> Result<(), Box<dyn std::error::Error>>
{
    // Attribute declared in child but NOT in base — not an override, must not fire
    let src = concat!(
        "class Base:\n",
        "    x: int = 0\n",
        "class Child(Base):\n",
        "    y: str = \"new\"\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0017").collect();
    assert!(e17.is_empty(), "attr only in child (not base) must not fire E0017");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0020 — Missing overload impl: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0020_single_overload_function_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Only one @overload definition (len < 2) — must not fire
    let src = "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n";
    let diags = run(src)?;
    let e20: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0020").collect();
    assert!(e20.is_empty(), "single @overload must not fire E0020 (< 2 defs)");
    Ok(())
}

#[test]
fn e0020_overloads_with_impl_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int: ...\n",
        "@overload\n",
        "def foo(x: str) -> str: ...\n",
        "def foo(x: int | str) -> int | str: return x\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0020").collect();
    assert!(e20.is_empty(), "@overload group with impl must not fire E0020");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0021 — Overlapping overloads: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0021_three_overlapping_overloads_emits_one_per_later() -> Result<(), Box<dyn std::error::Error>>
{
    // When there are 3 overlapping overloads, only emit one diagnostic per later overload.
    // The `break` in check_group ensures at most one diag per later overload even if it
    // overlaps multiple earlier ones.
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x) -> int: ...\n",
        "@overload\n",
        "def foo(x) -> str: ...\n",
        "@overload\n",
        "def foo(x) -> bytes: ...\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0021").collect();
    // overload[1] overlaps overload[0], overload[2] overlaps overload[0] → 2 E0021
    assert_eq!(e21.len(), 2, "three overlapping overloads must emit two E0021 diagnostics");
    Ok(())
}

#[test]
fn e0021_different_param_count_does_not_overlap() -> Result<(), Box<dyn std::error::Error>> {
    // Overloads with different param count cannot overlap
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int: ...\n",
        "@overload\n",
        "def foo(x: int, y: int) -> int: ...\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0021").collect();
    assert!(e21.is_empty(), "different param count must not fire E0021");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0025 — Missing @override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn e0025_method_with_override_decorator_does_not_fire() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base') -> None: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0025").collect();
    assert!(e25.is_empty(), "method with @override must not fire E0025");
    Ok(())
}

#[test]
fn e0025_base_not_in_module_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Child(ExternalBase):\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0025").collect();
    assert!(e25.is_empty(), "external base class must not fire E0025");
    Ok(())
}

#[test]
fn e0025_new_method_not_in_base_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Child adds a NEW method not present in base — not an override
    let src = concat!(
        "class Base:\n",
        "    def existing(self: 'Base') -> None: pass\n",
        "class Child(Base):\n",
        "    def brand_new(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0025").collect();
    assert!(e25.is_empty(), "new method not in base must not fire E0025");
    Ok(())
}
