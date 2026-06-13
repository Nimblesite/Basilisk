//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_mutant_alias`.

use super::common::resolve_src;

#[test]
fn alias_name_preserves_import_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Optional, Union\n".to_owned();
    let resolved = resolve_src(&src)?;
    let import = resolved
        .imports
        .iter()
        .find(|i| i.module == "typing")
        .ok_or("no typing import")?;
    assert!(
        import.names.contains(&"Optional".to_owned()),
        "Optional must be in import names"
    );
    assert!(
        import.names.contains(&"Union".to_owned()),
        "Union must be in import names"
    );
    Ok(())
}

#[test]
fn alias_name_single_name_is_correct() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from os.path import join\n".to_owned();
    let resolved = resolve_src(&src)?;
    let import = resolved
        .imports
        .iter()
        .find(|i| i.module == "os.path")
        .ok_or("no import")?;
    assert_eq!(
        import.names,
        vec!["join".to_owned()],
        "join must be preserved"
    );
    Ok(())
}

#[test]
fn alias_name_uses_asname_when_present() -> Result<(), Box<dyn std::error::Error>> {
    // Issues #107/#64: `from X import Y as Z` binds `Z`, not `Y`.
    let src = "from nap.api import auth as auth_mod\n".to_owned();
    let resolved = resolve_src(&src)?;
    let import = resolved
        .imports
        .iter()
        .find(|i| i.module == "nap.api")
        .ok_or("no import")?;
    assert_eq!(
        import.names,
        vec!["auth_mod".to_owned()],
        "the bound alias name must be recorded, not the original"
    );
    Ok(())
}
