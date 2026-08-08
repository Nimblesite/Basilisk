//! Implements [RESOLV-CANONICAL-BINDING]: the binding table is scope- and
//! order-correct.
//!
//! Pins the 2026-08-08 review findings against `src/binding.rs`:
//!
//! - imports inside `def`/`class` bodies bind THEIR scope, never the module's;
//! - module-level rebinding is positional — a later `Final = …` does not
//!   corrupt an earlier `Final` use, and an import after a rebind wins for
//!   later uses;
//! - `binds_name` sees plain `import x` bindings, not only `from` imports.

use basilisk_canonical::{BindingTable, CanonicalSymbol};
use ruff_python_ast::{Expr, ModModule, Stmt};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse Python source into a module AST.
fn parsed(source: &str) -> Result<ModModule, ruff_python_parser::ParseError> {
    Ok(ruff_python_parser::parse_module(source)?.into_syntax())
}

/// The annotation expression of the module-level `target: Ann = …` statement.
fn annotation_of<'a>(body: &'a [Stmt], target: &str) -> Option<&'a Expr> {
    body.iter().find_map(|stmt| {
        let Stmt::AnnAssign(assign) = stmt else {
            return None;
        };
        match assign.target.as_ref() {
            Expr::Name(name) if name.id.as_str() == target => Some(assign.annotation.as_ref()),
            _ => None,
        }
    })
}

/// Resolve the annotation of `target` through the module's binding table.
fn annotation_symbol(source: &str, target: &str) -> Result<Option<CanonicalSymbol>, String> {
    let module = parsed(source).map_err(|error| error.to_string())?;
    let table = BindingTable::from_module(&module.body);
    let annotation = annotation_of(&module.body, target)
        .ok_or_else(|| format!("no annotated assignment to {target} in fixture"))?;
    Ok(table.canonical_of(annotation))
}

/// An import inside a function body binds the FUNCTION scope. The module
/// never sees it, so a module-level use of the name resolves to nothing.
#[test]
fn function_body_import_does_not_leak_to_module_scope() -> TestResult {
    let source = "
def helper() -> None:
    from typing import Final

value: Final = 1
";
    assert_eq!(annotation_symbol(source, "value")?, None);
    Ok(())
}

/// An import inside a class body binds the CLASS namespace, not the module.
#[test]
fn class_body_import_does_not_leak_to_module_scope() -> TestResult {
    let source = "
class Config:
    from typing import Final

value: Final = 1
";
    assert_eq!(annotation_symbol(source, "value")?, None);
    Ok(())
}

/// Imports under module-level guards (`try`/`except ImportError`) DO bind at
/// module scope: those statements execute in the module's own frame.
#[test]
fn guarded_module_level_import_still_binds() -> TestResult {
    let source = "
try:
    from typing import Final
except ImportError:
    pass

value: Final = 1
";
    assert_eq!(
        annotation_symbol(source, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );
    Ok(())
}

/// A use BETWEEN the import and a later rebind refers to the import. The
/// later `Final = …` must not corrupt the earlier use.
#[test]
fn later_rebind_does_not_corrupt_earlier_use() -> TestResult {
    let source = "
from typing import Final

value: Final = 1
Final = object()
";
    assert_eq!(
        annotation_symbol(source, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );
    Ok(())
}

/// An import AFTER a rebind legitimately replaces it: a use after the import
/// refers to the imported definition.
#[test]
fn import_after_rebind_wins_for_later_uses() -> TestResult {
    let source = "
Final = object()
from typing import Final

value: Final = 1
";
    assert_eq!(
        annotation_symbol(source, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );
    Ok(())
}

/// A use AFTER a module-level rebind refers to the rebound name, never the
/// import.
#[test]
fn use_after_rebind_resolves_to_nothing() -> TestResult {
    let source = "
from typing import Final

Final = object()
value: Final = 1
";
    assert_eq!(annotation_symbol(source, "value")?, None);
    Ok(())
}

/// Module-level compound statements whose TARGETS bind names (`for`, `with
/// … as`, except-as, match captures) rebind them for later uses.
#[test]
fn compound_statement_targets_rebind() -> TestResult {
    let source = "
from typing import Final

for Final in range(3):
    pass

value: Final = 1
";
    assert_eq!(annotation_symbol(source, "value")?, None);
    Ok(())
}

/// `import x` and `import x as y` bind module names like any other binding;
/// `binds_name` must see them, and a relative import binds its local names.
#[test]
fn binds_name_sees_plain_module_imports() -> TestResult {
    let source = "
import json
import numpy as np
from . import siblings
";
    let module = parsed(source)?;
    let table = BindingTable::from_module(&module.body);
    assert!(table.binds_name("json"), "`import json` binds `json`");
    assert!(table.binds_name("np"), "`import numpy as np` binds `np`");
    assert!(
        !table.binds_name("numpy"),
        "`import numpy as np` binds the asname, not `numpy`"
    );
    assert!(
        table.binds_name("siblings"),
        "`from . import siblings` binds `siblings`"
    );
    Ok(())
}

/// The alias / dotted / shadow trio keeps resolving exactly as before the
/// rework: aliased from-imports and module-attribute access resolve, a local
/// class shadows the name for later uses.
#[test]
fn alias_dotted_and_shadow_still_resolve() -> TestResult {
    let aliased = "
from typing import Final as F

value: F = 1
";
    assert_eq!(
        annotation_symbol(aliased, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );

    let dotted = "
import typing as t

value: t.Final = 1
";
    assert_eq!(
        annotation_symbol(dotted, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );

    let shadowed = "
class Final:
    pass

value: Final = 1
";
    assert_eq!(annotation_symbol(shadowed, "value")?, None);
    Ok(())
}

/// A star import from a registry module binds that module's specification
/// names for uses after it, and a later explicit rebind still wins.
#[test]
fn star_import_binds_positionally() -> TestResult {
    let source = "
from typing import *

value: Final = 1
Final = object()
after: Final = 2
";
    assert_eq!(
        annotation_symbol(source, "value")?,
        Some(CanonicalSymbol::new("typing", "Final"))
    );
    assert_eq!(annotation_symbol(source, "after")?, None);
    Ok(())
}
