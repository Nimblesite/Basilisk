//! Tests for resolver: test_mutant_typeddict.

mod common;

use common::resolve_src;

#[test]
fn collect_typeddict_calls_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        r#"Movie = TypedDict("Movie", {"name": str, "year": int})"#,
        "\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "TypedDict functional call must be collected"
    );
    let td = &resolved.typeddict_calls[0];
    assert_eq!(td.lhs_name, "Movie");
    Ok(())
}

#[test]
fn collect_typeddict_calls_only_matches_typeddict_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        r#"NotTypedDict = dict("Name", {"x": int})"#,
        "\n",
        r#"Movie = TypedDict("Movie", {"name": str})"#,
        "\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "only TypedDict call must be collected, not dict"
    );
    assert_eq!(resolved.typeddict_calls[0].lhs_name, "Movie");
    Ok(())
}

#[test]
fn collect_typeddict_calls_qualified_typing_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        r#"Movie = typing.TypedDict("Movie", {"name": str})"#,
        "\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "typing.TypedDict must be collected"
    );
    assert_eq!(resolved.typeddict_calls[0].lhs_name, "Movie");
    Ok(())
}

#[test]
fn collect_typeddict_calls_non_string_key_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(r#"Movie = TypedDict("Movie", {1: str})"#, "\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        resolved.typeddict_calls[0].has_non_string_key,
        "non-string dict key must set has_non_string_key=true"
    );
    Ok(())
}

#[test]
fn collect_typeddict_calls_string_keys_only() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        r#"Movie = TypedDict("Movie", {"name": str, "year": int})"#,
        "\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        !resolved.typeddict_calls[0].has_non_string_key,
        "all-string keys must set has_non_string_key=false"
    );
    Ok(())
}

#[test]
fn collect_typeddict_calls_non_dict_second_arg() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::TypedDictSecondArgKind;
    let src = concat!(
        "fields = {'name': str}\n",
        r#"Movie = TypedDict("Movie", fields)"#,
        "\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert_eq!(
        resolved.typeddict_calls[0].second_arg_kind,
        TypedDictSecondArgKind::NotDictLiteral,
        "variable second arg must produce NotDictLiteral"
    );
    Ok(())
}

#[test]
fn collect_typeddict_calls_keyword_only_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(r#"Movie = TypedDict("Movie", name=str, year=int)"#, "\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        !resolved.typeddict_calls[0].has_positional_dict,
        "keyword-only form must set has_positional_dict=false"
    );
    Ok(())
}
