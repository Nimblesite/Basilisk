//! Tests for resolver: test_namedtuple.

mod common;

use common::resolve_src;

#[test]
fn namedtuple_typing_form_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NamedTuple\n",
        "Point = NamedTuple('Point', [('x', int), ('y', int)])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].lhs_name, "Point");
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    assert!(resolved.namedtuple_defs[0].has_types);
    Ok(())
}

#[test]
fn namedtuple_collections_form_string_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', 'x y')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    assert!(!resolved.namedtuple_defs[0].has_types);
    Ok(())
}

#[test]
fn namedtuple_collections_form_comma_string() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', 'x, y, z')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y", "z"]);
    Ok(())
}

#[test]
fn namedtuple_collections_form_list_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', ['x', 'y'])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_collections_form_tuple_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', ('x', 'y'))\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_rename_true_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', ['x', 'y'], rename=True)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.namedtuple_defs.is_empty(),
        "namedtuple with rename=True must be skipped"
    );
    Ok(())
}

#[test]
fn namedtuple_typing_form_tuple_pairs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NamedTuple\n",
        "Point = NamedTuple('Point', (('x', int), ('y', int)))\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.namedtuple_defs.len(), 1);
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_typing_list_of_pairs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NamedTuple\n",
        "Point = NamedTuple('Point', [('x', int), ('y', int)])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.namedtuple_defs.is_empty(),
        "NamedTuple def should be collected"
    );
    let nt = &resolved.namedtuple_defs[0];
    assert_eq!(nt.field_names, vec!["x", "y"]);
    assert!(nt.has_types);
    Ok(())
}

#[test]
fn namedtuple_typing_tuple_of_pairs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NamedTuple\n",
        "Point = NamedTuple('Point', (('x', int), ('y', int)))\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.namedtuple_defs.is_empty());
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_collections_string_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', 'x y')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.namedtuple_defs.is_empty());
    let nt = &resolved.namedtuple_defs[0];
    assert_eq!(nt.field_names, vec!["x", "y"]);
    assert!(!nt.has_types);
    Ok(())
}

#[test]
fn namedtuple_collections_list_of_strings() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', ['x', 'y'])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.namedtuple_defs.is_empty());
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_with_defaults_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from collections import namedtuple\n",
        "Point = namedtuple('Point', ['x', 'y', 'z'], defaults=(0, 0))\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.namedtuple_defs.is_empty());
    assert_eq!(resolved.namedtuple_defs[0].defaults_count, 2);
    Ok(())
}

#[test]
fn namedtuple_final_string_constant_resolved() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "from collections import namedtuple\n",
        "X: Final = 'x'\n",
        "Y: Final = 'y'\n",
        "Point = namedtuple('Point', [X, Y])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.namedtuple_defs.is_empty());
    assert_eq!(resolved.namedtuple_defs[0].field_names, vec!["x", "y"]);
    Ok(())
}

#[test]
fn namedtuple_functional_3_fields() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import NamedTuple\nPoint3D = NamedTuple('Point3D', [('x', int), ('y', int), ('z', int)])\n".to_owned();
    let resolved = resolve_src(&src)?;
    let nt = resolved
        .namedtuple_defs
        .iter()
        .find(|n| n.lhs_name == "Point3D");
    assert!(nt.is_some_and(|n| n.field_names.len() == 3));
    Ok(())
}

#[test]
fn dc_transform_overloaded_field_spec() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform, overload\n",
        "@overload\n",
        "def field(*, init: bool = True, kw_only: bool = False) -> object: ...\n",
        "@overload\n",
        "def field(default: object, init: bool = True, kw_only: bool = False) -> object: ...\n",
        "def field(*args: object, **kwargs: object) -> object: ...\n",
        "@dataclass_transform(field_specifiers=(field,))\n",
        "class ModelBase:\n",
        "    pass\n",
        "class User(ModelBase):\n",
        "    name: str = field(init=True)\n",
        "    hidden: int = field(kw_only=True)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "User"));
    Ok(())
}

#[test]
fn dc_transform_field_spec_positional_init() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "def myfield(init: bool = True, kw_only: bool = False, default: object = ...) -> object: ...\n",
        "@dataclass_transform(field_specifiers=(myfield,))\n",
        "class Base:\n",
        "    pass\n",
        "class Item(Base):\n",
        "    val: int = myfield(True, True)\n",
    ).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Item"));
    Ok(())
}

#[test]
fn multiple_unbounded_starred_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVarTuple\n",
        "Ts = TypeVarTuple('Ts')\n",
        "Us = TypeVarTuple('Us')\n",
        "def f(x: tuple[*Ts, *Us]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.multiple_unbounded_tuple_spans.is_empty());
    Ok(())
}

#[test]
fn base_class_call_expr_refs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def make_base() -> type: ...\n",
        "class Child(make_base()):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Child"));
    Ok(())
}

#[test]
fn bounded_typevar_return_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> str:\n",
        "        return val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_assign_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x = val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}
