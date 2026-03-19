// Tests for resolver: `test_match`.

use super::common::resolve_src;

#[test]
fn resolves_match_statement_with_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 1\nmatch x:\n    case 1:\n        pass\n    case _:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(
        resolved
            .match_stmts
            .first()
            .expect("expected at least one match stmt")
            .has_wildcard,
        "case _ must set has_wildcard"
    );
    Ok(())
}

#[test]
fn resolves_match_with_or_wildcard_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 1\nmatch x:\n    case 1 | _:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(
        resolved
            .match_stmts
            .first()
            .expect("expected at least one match stmt")
            .has_wildcard,
        "case 1 | _ must be recognised as wildcard via MatchOr"
    );
    Ok(())
}

#[test]
fn match_without_wildcard_has_no_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 1\nmatch x:\n    case 1:\n        pass\n    case 2:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(
        !resolved
            .match_stmts
            .first()
            .expect("expected at least one match stmt")
            .has_wildcard
    );
    Ok(())
}

#[test]
fn match_stmt_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 5\n",
        "match x:\n",
        "    case 1:\n",
        "        pass\n",
        "    case _:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.match_stmts.is_empty());
    Ok(())
}
