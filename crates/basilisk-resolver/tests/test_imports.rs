mod common;

use common::{resolve_src};

#[test]
fn resolves_star_import() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    let src = "from os.path import *\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.imports.len(), 1);
    assert_eq!(resolved.imports[0].kind, ImportKind::Star);
    Ok(())
}

#[test]
fn resolves_plain_import() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    let src = "import os\nimport sys\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.imports.len(), 2);
    assert!(resolved.imports.iter().all(|i| i.kind == ImportKind::Plain));
    Ok(())
}

#[test]
fn resolves_from_import_without_module_name() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    // Relative import: `from . import utils` — module field will be empty
    let src = "from . import utils\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.imports.len(), 1);
    assert_eq!(resolved.imports[0].kind, ImportKind::From);
    assert_eq!(resolved.imports[0].module, "");
    Ok(())
}
