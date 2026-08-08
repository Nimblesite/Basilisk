//! Implements [RESOLV-CANONICAL-RELATION]: semantic type-expression relations.
//!
//! Every case is derived from the typing specification, never from a
//! conformance fixture:
//!
//! - Equivalence is mutual assignability
//!   (<https://typing.python.org/en/latest/spec/glossary.html#term-equivalent>).
//! - `Any` is consistent with every type
//!   (<https://typing.python.org/en/latest/spec/concepts.html#the-any-type>).
//! - `bool` < `int` < `float` < `complex` by the numeric-tower special case
//!   (<https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex>).
//! - `Literal[v]` is assignable to `type(v)`; two literals are equivalent only
//!   when their values have the same type and equal value
//!   (<https://typing.python.org/en/latest/spec/literal.html>).
//!
//! The relations answer `Option<bool>`: `None` is honest abstention, never a
//! verdict. A relation the layer cannot decide from resolved nodes must
//! abstain rather than guess.

use basilisk_canonical::{assignable, equivalent, BindingTable, TypeNode};
use ruff_python_ast::{Expr, ModModule, Stmt};

/// Parse Python source into a module AST.
fn parsed(source: &str) -> Result<ModModule, ruff_python_parser::ParseError> {
    Ok(ruff_python_parser::parse_module(source)?.into_syntax())
}

/// Lower the annotation of the LAST module-level `name: annotation` statement.
fn lowered(module: &ModModule, bindings: &BindingTable) -> Option<TypeNode> {
    module.body.iter().rev().find_map(|stmt| {
        let Stmt::AnnAssign(ann) = stmt else {
            return None;
        };
        matches!(ann.target.as_ref(), Expr::Name(_))
            .then(|| TypeNode::lower(bindings, &ann.annotation))
    })
}

/// Lower the annotations of the last two `name: annotation` statements, in
/// source order, and relate them as (source, target).
fn lower_pair(source_py: &str) -> Result<(TypeNode, TypeNode), String> {
    let module = parsed(source_py).map_err(|error| error.to_string())?;
    let bindings = BindingTable::from_module(&module.body);
    let mut nodes: Vec<TypeNode> = module
        .body
        .iter()
        .filter_map(|stmt| {
            let Stmt::AnnAssign(ann) = stmt else {
                return None;
            };
            Some(TypeNode::lower(&bindings, &ann.annotation))
        })
        .collect();
    let target = nodes.pop().ok_or("fixture lacks a target annotation")?;
    let source = nodes.pop().ok_or("fixture lacks a source annotation")?;
    Ok((source, target))
}

/// Assert `assignable(source, target)` over a two-annotation fixture.
fn assert_assignable(source_py: &str, expected: Option<bool>) {
    match lower_pair(source_py) {
        Ok((source, target)) => {
            assert_eq!(
                assignable(&source, &target),
                expected,
                "assignable over fixture:\n{source_py}"
            );
        }
        Err(error) => panic!("{error}"),
    }
}

/// Assert `equivalent(a, b)` over a two-annotation fixture.
fn assert_equivalent(source_py: &str, expected: Option<bool>) {
    match lower_pair(source_py) {
        Ok((a, b)) => {
            assert_eq!(
                equivalent(&a, &b),
                expected,
                "equivalent over fixture:\n{source_py}"
            );
        }
        Err(error) => panic!("{error}"),
    }
}

// ---------------------------------------------------------------------------
// Lowering resolves meaning, not spelling
// ---------------------------------------------------------------------------

#[test]
fn qualified_and_bare_builtin_lower_identically() {
    let bare = parsed("x: int\n").expect("parse");
    let qualified = parsed("import builtins\nx: builtins.int\n").expect("parse");
    let bare_node = lowered(&bare, &BindingTable::from_module(&bare.body));
    let qualified_node = lowered(&qualified, &BindingTable::from_module(&qualified.body));
    assert_eq!(bare_node, qualified_node);
    assert!(bare_node.is_some());
}

#[test]
fn aliased_optional_lowers_like_pep604_union() {
    let aliased = parsed("from typing import Optional as Opt\nx: Opt[int]\n").expect("parse");
    let pep604 = parsed("x: int | None\n").expect("parse");
    let aliased_node =
        lowered(&aliased, &BindingTable::from_module(&aliased.body)).expect("lowered");
    let union_node = lowered(&pep604, &BindingTable::from_module(&pep604.body)).expect("lowered");
    assert_eq!(equivalent(&aliased_node, &union_node), Some(true));
}

#[test]
fn shadowed_builtin_is_not_the_builtin() {
    let module = parsed("class int:\n    pass\nx: int\n").expect("parse");
    let node = lowered(&module, &BindingTable::from_module(&module.body)).expect("lowered");
    // A local `class int:` is a user class this layer cannot relate; every
    // relation against it must abstain rather than treat it as the builtin.
    assert_eq!(assignable(&node, &node), None);
}

// ---------------------------------------------------------------------------
// The numeric tower and object
// ---------------------------------------------------------------------------

#[test]
fn numeric_tower_special_cases_are_assignable() {
    assert_assignable("x: bool\ny: int\n", Some(true));
    assert_assignable("x: int\ny: float\n", Some(true));
    assert_assignable("x: int\ny: complex\n", Some(true));
    assert_assignable("x: float\ny: complex\n", Some(true));
}

#[test]
fn numeric_tower_is_directional() {
    assert_assignable("x: float\ny: int\n", Some(false));
    assert_assignable("x: complex\ny: float\n", Some(false));
    assert_assignable("x: int\ny: bool\n", Some(false));
}

#[test]
fn every_decided_type_is_assignable_to_object() {
    assert_assignable("x: int\ny: object\n", Some(true));
    assert_assignable("x: str | bytes\ny: object\n", Some(true));
    assert_assignable("x: None\ny: object\n", Some(true));
    assert_assignable("x: object\ny: int\n", Some(false));
}

#[test]
fn unrelated_builtin_classes_are_not_assignable() {
    assert_assignable("x: str\ny: int\n", Some(false));
    assert_assignable("x: bytes\ny: str\n", Some(false));
    // PEP 688 removed the bytearray-to-bytes promotion.
    assert_assignable("x: bytearray\ny: bytes\n", Some(false));
}

// ---------------------------------------------------------------------------
// Any, Never, None
// ---------------------------------------------------------------------------

#[test]
fn any_is_consistent_in_both_directions() {
    assert_assignable("from typing import Any\nx: Any\ny: int\n", Some(true));
    assert_assignable("from typing import Any\nx: int\ny: Any\n", Some(true));
}

#[test]
fn never_is_the_bottom_type() {
    assert_assignable("from typing import Never\nx: Never\ny: int\n", Some(true));
    assert_assignable("from typing import Never\nx: int\ny: Never\n", Some(false));
}

#[test]
fn none_relates_only_to_none_object_and_optionals() {
    assert_assignable("x: None\ny: None\n", Some(true));
    assert_assignable("x: None\ny: int\n", Some(false));
    assert_assignable("x: None\ny: int | None\n", Some(true));
    assert_assignable(
        "from typing import Optional\nx: None\ny: Optional[str]\n",
        Some(true),
    );
}

// ---------------------------------------------------------------------------
// Unions
// ---------------------------------------------------------------------------

#[test]
fn union_member_is_assignable_to_the_union() {
    assert_assignable("x: int\ny: int | str\n", Some(true));
}

#[test]
fn union_source_requires_every_member_accepted() {
    assert_assignable("x: int | str\ny: int\n", Some(false));
    assert_assignable("x: int | bool\ny: int\n", Some(true));
}

#[test]
fn typing_union_and_pep604_union_are_equivalent() {
    assert_equivalent(
        "from typing import Union\nx: Union[int, str]\ny: str | int\n",
        Some(true),
    );
}

// ---------------------------------------------------------------------------
// Literals
// ---------------------------------------------------------------------------

#[test]
fn literal_is_assignable_to_its_value_type() {
    assert_assignable(
        "from typing import Literal\nx: Literal[3]\ny: int\n",
        Some(true),
    );
    assert_assignable(
        "from typing import Literal\nx: Literal[3]\ny: float\n",
        Some(true),
    );
    assert_assignable(
        "from typing import Literal\nx: Literal['a']\ny: str\n",
        Some(true),
    );
    assert_assignable(
        "from typing import Literal\nx: Literal[3]\ny: str\n",
        Some(false),
    );
}

#[test]
fn literal_equivalence_requires_same_value_type() {
    // type(True) is bool, type(1) is int: not equivalent despite True == 1.
    assert_equivalent(
        "from typing import Literal\nx: Literal[True]\ny: Literal[1]\n",
        Some(false),
    );
    assert_equivalent(
        "from typing import Literal\nx: Literal[3]\ny: Literal[3]\n",
        Some(true),
    );
}

#[test]
fn multi_value_literal_is_the_union_of_its_values() {
    assert_assignable(
        "from typing import Literal\nx: Literal[1, 2]\ny: int\n",
        Some(true),
    );
    assert_assignable(
        "from typing import Literal\nx: Literal[1, 'a']\ny: int\n",
        Some(false),
    );
    // Literal[None] is equivalent to None.
    assert_assignable(
        "from typing import Literal\nx: Literal[None]\ny: None\n",
        Some(true),
    );
}

#[test]
fn literal_string_sits_between_str_literals_and_str() {
    assert_assignable(
        "from typing import Literal, LiteralString\nx: Literal['a']\ny: LiteralString\n",
        Some(true),
    );
    assert_assignable(
        "from typing import LiteralString\nx: LiteralString\ny: str\n",
        Some(true),
    );
    assert_assignable(
        "from typing import LiteralString\nx: str\ny: LiteralString\n",
        Some(false),
    );
}

// ---------------------------------------------------------------------------
// Parameterized builtins
// ---------------------------------------------------------------------------

#[test]
fn identical_parameterizations_are_equivalent() {
    assert_equivalent("x: list[int]\ny: list[int]\n", Some(true));
    assert_equivalent("x: dict[str, int]\ny: dict[str, int]\n", Some(true));
}

#[test]
fn invariant_containers_reject_different_parameters() {
    assert_assignable("x: list[int]\ny: list[str]\n", Some(false));
    // Invariance: int < float does not lift into list.
    assert_assignable("x: list[int]\ny: list[float]\n", Some(false));
}

#[test]
fn any_parameter_is_consistent_with_every_parameter() {
    assert_assignable("from typing import Any\nx: list[Any]\ny: list[int]\n", Some(true));
    assert_assignable("from typing import Any\nx: list[int]\ny: list[Any]\n", Some(true));
}

#[test]
fn bare_container_means_any_parameters() {
    assert_assignable("x: list[int]\ny: list\n", Some(true));
    assert_assignable("x: list\ny: list[int]\n", Some(true));
}

#[test]
fn deprecated_capitalized_alias_is_the_builtin_class() {
    assert_equivalent(
        "from typing import List\nx: List[int]\ny: list[int]\n",
        Some(true),
    );
}

#[test]
fn tuple_parameters_are_covariant() {
    assert_assignable("x: tuple[int, str]\ny: tuple[float, str]\n", Some(true));
    assert_assignable("x: tuple[int, str]\ny: tuple[int]\n", Some(false));
}

// ---------------------------------------------------------------------------
// Qualifiers and metadata that wrap a type
// ---------------------------------------------------------------------------

#[test]
fn annotated_relates_as_its_first_argument() {
    assert_assignable(
        "from typing import Annotated\nx: Annotated[int, 'meta']\ny: int\n",
        Some(true),
    );
}

// ---------------------------------------------------------------------------
// Honest abstention
// ---------------------------------------------------------------------------

#[test]
fn unresolved_names_abstain() {
    assert_assignable("x: Foo\ny: int\n", None);
    assert_assignable("x: int\ny: Foo\n", None);
}

#[test]
fn unmodelled_relations_abstain_rather_than_deny() {
    // list[int] IS assignable to Sequence[int], but this layer does not model
    // protocol subtyping yet; the only honest answer is abstention.
    assert_assignable(
        "from typing import Sequence\nx: list[int]\ny: Sequence[int]\n",
        None,
    );
    // A quoted annotation is unresolved here; abstain.
    assert_assignable("x: 'int'\ny: int\n", None);
}
