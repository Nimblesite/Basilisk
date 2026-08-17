//! Tests for [RESOLV-CANONICAL-BINDING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//!
//! Identity pins for the variable→`TypedDict` association. The deleted
//! predecessor joined `m: Movie` to `class Movie` by SPELLING, so every test
//! here is a shape that join got wrong: an aliased annotation, a quoted
//! forward reference, a rebound name, a same-named ordinary class, a dotted
//! import, and builtin resolution for the value-class check. None of these
//! shapes exists in the conformance suite.

use super::common::resolve_src;
use basilisk_resolver::TypedDictKeyViolationKind;

fn has_invalid_literal_key(resolved: &basilisk_resolver::ResolvedModule, key: &str) -> bool {
    resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            TypedDictKeyViolationKind::InvalidDictLiteral { invalid_keys, .. }
                if invalid_keys.iter().any(|k| k == key)
        )
    })
}

fn has_missing_literal_key(resolved: &basilisk_resolver::ResolvedModule, key: &str) -> bool {
    resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            TypedDictKeyViolationKind::InvalidDictLiteral { missing_keys, .. }
                if missing_keys.iter().any(|k| k == key)
        )
    })
}

fn has_wrong_value_type(resolved: &basilisk_resolver::ResolvedModule, key: &str) -> bool {
    resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            TypedDictKeyViolationKind::WrongSubscriptValueType { key: k, .. } if k == key
        )
    })
}

// An assignment alias binds the same class object, so the schema must reach
// a variable annotated through it (PEP 589 via the binding table).
#[test]
fn typeddict_schema_reaches_variable_through_assignment_alias(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "Alias = Movie\n",
        "m: Alias = {'titel': 'x'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        has_invalid_literal_key(&resolved, "titel"),
        "a TypedDict reached through an assignment alias must validate keys"
    );
    assert!(
        has_missing_literal_key(&resolved, "title"),
        "the aliased schema's required key must be reported missing"
    );
    Ok(())
}

// PEP 484: a quoted annotation is a forward reference to the same class.
#[test]
fn typeddict_schema_reaches_variable_through_quoted_annotation(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "m: \"Movie\" = {'titel': 'x'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        has_invalid_literal_key(&resolved, "titel"),
        "a TypedDict reached through a quoted forward reference must validate keys"
    );
    Ok(())
}

// A name rebound away from the TypedDict no longer denotes it; validating the
// old schema against the new annotation would be a spelling verdict.
#[test]
fn rebound_typeddict_name_stops_carrying_the_schema() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "Movie = dict\n",
        "m: Movie = {'anything': 1}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.is_empty(),
        "after `Movie = dict`, the annotation no longer denotes the TypedDict: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// Two classes spelled alike are two nodes. The annotation between the two
// definitions denotes the TypedDict; the one after the ordinary class does
// not.
#[test]
fn same_named_ordinary_class_does_not_inherit_the_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "before: Movie = {'titel': 'x'}\n",
        "class Movie:\n",
        "    pass\n",
        "after: Movie = {'titel': 'x'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let invalid_key_count = resolved
        .typeddict_key_violations
        .iter()
        .filter(|v| {
            matches!(
                &v.kind,
                TypedDictKeyViolationKind::InvalidDictLiteral { invalid_keys, .. }
                    if invalid_keys.iter().any(|k| k == "titel")
            )
        })
        .count();
    assert_eq!(
        invalid_key_count, 1,
        "exactly the annotation BEFORE the rebinding denotes the TypedDict: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// A class in another module is not a class this module defines; the honest
// answer is abstention, never a schema borrowed from a same-spelled local.
#[test]
fn dotted_annotation_from_another_module_is_abstention() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "import other\n",
        "m: other.Movie = {'anything': 1}\n",
        "m['whatever'] = 2\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.is_empty(),
        "a class from another module has no local schema: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// The parameter association travels the same resolved route as variables.
#[test]
fn parameter_annotated_through_alias_carries_the_schema() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "MovieAlias = Movie\n",
        "def f(m: MovieAlias) -> None:\n",
        "    m['bad'] = 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.iter().any(|v| matches!(
            &v.kind,
            TypedDictKeyViolationKind::InvalidSubscriptKey { key } if key == "bad"
        )),
        "a parameter annotated through an alias must carry the schema: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// PEP 692: `**kwargs: Unpack[Movie]` types the mapping with the schema, and
// the `Unpack` head itself resolves through the bindings.
#[test]
fn kwargs_unpack_reaches_the_schema_through_an_alias() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Unpack as U\n",
        "class Movie(TypedDict):\n",
        "    title: str\n",
        "def f(**kwargs: U[Movie]) -> None:\n",
        "    kwargs['bad'] = 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.iter().any(|v| matches!(
            &v.kind,
            TypedDictKeyViolationKind::InvalidSubscriptKey { key } if key == "bad"
        )),
        "kwargs typed via an aliased Unpack must carry the schema: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// PEP 705 read-only enforcement travels the same identity join.
#[test]
fn readonly_field_reached_through_alias_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, ReadOnly\n",
        "class Album(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "AlbumAlias = Album\n",
        "a: AlbumAlias = {'name': 'x'}\n",
        "a['name'] = 'y'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.readonly_violations.is_empty(),
        "a ReadOnly field reached through an alias must be protected"
    );
    Ok(())
}

// PEP 655: `NotRequired` in a total TypedDict lifts the requiredness.
#[test]
fn notrequired_field_is_not_missing_in_total_typeddict() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import TypedDict, NotRequired\n",
        "class Config(TypedDict):\n",
        "    host: str\n",
        "    port: NotRequired[int]\n",
        "c: Config = {'host': 'x'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !has_missing_literal_key(&resolved, "port"),
        "a NotRequired field must not be reported missing: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// PEP 655: `Required` in a `total=False` TypedDict restores the requiredness,
// and the qualifier resolves through an aliased import.
#[test]
fn aliased_required_qualifier_restores_requiredness_under_total_false(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Required as R\n",
        "class Config(TypedDict, total=False):\n",
        "    host: R[str]\n",
        "    port: int\n",
        "c: Config = {}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        has_missing_literal_key(&resolved, "host"),
        "an aliased Required qualifier must restore requiredness: {:?}",
        resolved.typeddict_key_violations
    );
    assert!(
        !has_missing_literal_key(&resolved, "port"),
        "an unqualified field under total=False stays optional: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// PEP 655: an inherited field keeps the totality of the class that DECLARES
// it — a `total=False` base's fields stay optional inside a total subclass.
#[test]
fn inherited_field_keeps_declaring_class_totality() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Base(TypedDict, total=False):\n",
        "    x: int\n",
        "class Sub(Base):\n",
        "    y: str\n",
        "s: Sub = {'y': 'a'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !has_missing_literal_key(&resolved, "x"),
        "a field declared under total=False stays optional when inherited: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// The value-class check resolves the field's annotation through the bindings,
// so an aliased builtin import answers like the builtin.
#[test]
fn value_class_check_survives_aliased_builtin_import() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from builtins import int as I\n",
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    year: I\n",
        "movie: Movie = {'year': 2024}\n",
        "movie['year'] = 'wrong'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        has_wrong_value_type(&resolved, "year"),
        "an aliased builtin annotation must still be judged: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// A module-local `class int` shadows the builtin; judging the field against
// the BUILTIN `int` anyway would be a spelling verdict. Abstention is the
// only honest answer.
#[test]
fn value_class_check_abstains_when_builtin_is_shadowed() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import TypedDict\n",
        "class int:\n",
        "    pass\n",
        "class Movie(TypedDict):\n",
        "    year: int\n",
        "movie: Movie = {'year': 2024}\n",
        "movie['year'] = 'wrong'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !has_wrong_value_type(&resolved, "year"),
        "a shadowed builtin is not the builtin; the field is not judgeable: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}

// A union member that is `None` is read from its own node, and the union
// accepts either side.
#[test]
fn union_with_none_accepts_both_sides() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    year: int | None\n",
        "movie: Movie = {'year': 2024}\n",
        "movie['year'] = None\n",
        "movie['year'] = 'wrong'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong_count = resolved
        .typeddict_key_violations
        .iter()
        .filter(|v| {
            matches!(
                &v.kind,
                TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. } if key == "year"
            )
        })
        .count();
    assert_eq!(
        wrong_count, 1,
        "None is accepted, 'wrong' is not: {:?}",
        resolved.typeddict_key_violations
    );
    Ok(())
}
