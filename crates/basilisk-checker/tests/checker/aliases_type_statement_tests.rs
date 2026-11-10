//! Tests for [`aliases_type_statement`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for aliases_type_statement: PEP 695 type alias invalid RHS.
//
// The invalid forms mirror conformance `aliases_type_statement.py`
// (`BadTypeAlias1`–`BadTypeAlias13`): the rule must fire on every one of
// them and stay silent on every valid type expression.

use super::common::*;

fn fires(source: &str) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(codes(&run(source)?).contains(&"aliases_type_statement"))
}

#[test]
fn valid_aliases_do_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Vector = list[float]
type Matrix = list[Vector]
type Pair[T] = tuple[T, T]
type MaybeInt = int | None
type Forward = "Vector"
type Dotted = collections.abc.Sequence
"#;
    assert!(!fires(source)?, "valid type expressions must not fire");
    Ok(())
}

/// Every `BadTypeAlias1`–`BadTypeAlias13` form from the conformance suite.
#[test]
fn conformance_bad_alias_forms_all_fire() -> Result<(), Box<dyn std::error::Error>> {
    let var_prefix = "var1 = 3\n";
    let bad_forms = [
        "type Bad = eval(\"int\")",           // BadTypeAlias1: call
        "type Bad = [int, str]",              // BadTypeAlias2: list literal
        "type Bad = ((int, str),)",           // BadTypeAlias3: tuple literal
        "type Bad = [int for i in range(1)]", // BadTypeAlias4: comprehension
        "type Bad = {\"a\": \"b\"}",          // BadTypeAlias5: dict literal
        "type Bad = (lambda: int)()",         // BadTypeAlias6: lambda call
        "type Bad = [int][0]",                // BadTypeAlias7: subscripted list
        "type Bad = int if 1 < 3 else str",   // BadTypeAlias8: conditional
        "type Bad = var1",                    // BadTypeAlias9: non-type variable
        "type Bad = True",                    // BadTypeAlias10: bool literal
        "type Bad = 1",                       // BadTypeAlias11: int literal
        "type Bad = list or set",             // BadTypeAlias12: boolean op
        "type Bad = f\"{'int'}\"",            // BadTypeAlias13: f-string
    ];
    for form in bad_forms {
        let source = format!("{var_prefix}{form}\n");
        assert!(fires(&source)?, "must fire on: {form}");
    }
    Ok(())
}

#[test]
fn more_invalid_expression_forms_fire() -> Result<(), Box<dyn std::error::Error>> {
    for form in [
        "type Bad = -1",          // unary minus
        "type Bad = lambda: int", // bare lambda
        "type Bad = (int, str)",  // parenthesized tuple
    ] {
        let source = format!("{form}\n");
        assert!(fires(&source)?, "must fire on: {form}");
    }
    Ok(())
}

// ---- Issue #379: substring matching produced both misses and FPs ----

/// A perfectly valid alias to a class whose NAME contains "lambda" must not
/// fire — `rhs.contains("lambda")` was a substring false positive.
#[test]
fn identifier_containing_lambda_substring_is_not_flagged() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class Blambda:
    pass

type Alias = Blambda
";
    assert!(
        !fires(source)?,
        "an identifier containing the substring 'lambda' is a valid RHS"
    );
    Ok(())
}

/// A parenthesized conditional expression is still a conditional — the
/// text-level top-level-token scan missed it inside the parens.
#[test]
fn parenthesized_conditional_rhs_fires() -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        fires("type Bad = (int if True else str)\n")?,
        "a conditional stays invalid when parenthesized"
    );
    Ok(())
}

/// Any call is an invalid type expression, not just ones spelled `eval(`.
#[test]
fn call_rhs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def make() -> type:
    return int

type Bad = make()
";
    assert!(fires(source)?, "a call RHS is not a type expression");
    Ok(())
}

/// A comparison is an invalid type expression.
#[test]
fn comparison_rhs_fires() -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        fires("type Bad = int < str\n")?,
        "a comparison RHS is not a type expression"
    );
    Ok(())
}

/// A bytes literal is an invalid type expression (only str forward
/// references are permitted).
#[test]
fn bytes_literal_rhs_fires() -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        fires("type Bad = b\"int\"\n")?,
        "a bytes literal RHS is not a type expression"
    );
    Ok(())
}

/// The statement's own type parameters shadow module-level bindings inside
/// the RHS (PEP 695 annotation scope): `T = 1` must not make `T` invalid in
/// `type Wrapper[T] = ...` — the RHS `T` is the type parameter, not the
/// module variable.
#[test]
fn alias_own_type_parameter_shadowing_a_module_var_is_not_flagged(
) -> Result<(), Box<dyn std::error::Error>> {
    for form in [
        "T = 1\ntype Wrapper[T] = T | None\n",
        "T = 1\ntype Alias[T] = T\n",
        "T = 1\ntype Boxed[T] = list[T]\n",
    ] {
        assert!(
            !fires(form)?,
            "the alias's own type parameter must shadow the module var: {form}"
        );
    }
    Ok(())
}

/// The shadowing is per-statement: a DIFFERENT alias without that type
/// parameter still sees the non-type module binding.
#[test]
fn non_type_module_var_still_fires_without_the_shadowing_param(
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        fires("T = 1\ntype Wrapper[U] = U | None\ntype Bad = T\n")?,
        "an alias without the `T` parameter still sees the non-type `T = 1`"
    );
    Ok(())
}

/// Special-form subscript ARGUMENTS legitimately contain literals, lists,
/// and ellipses — the validator must never descend into them.
#[test]
fn special_form_subscript_args_are_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated, Callable, Literal

type Lit = Literal[5, "on", True]
type Fn = Callable[[int, str], bool]
type Meta = Annotated[int, {"units": "m"}]
type Row = tuple[int, ...]
"#;
    assert!(
        !fires(source)?,
        "special-form subscript arguments are valid type expressions"
    );
    Ok(())
}
