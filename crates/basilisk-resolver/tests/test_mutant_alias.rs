mod common;

use common::resolve_src;

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
