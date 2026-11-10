//! Implements [RESOLV-CANONICAL-BINDING]: builtin fallback is POSITIONAL.
//!
//! Pins the 2026-08-08 review finding against `src/binding.rs`:
//! `form_of_with_builtins` consulted the existential `binds_name`, so a
//! module-level rebinding ANYWHERE in the module suppressed builtin
//! recognition at use sites the rebinding cannot govern. The binding table
//! records positional events; the builtin fallback must use them: a bare
//! name refers to the builtin exactly while no module-level binding at or
//! before the use site governs it.

use basilisk_canonical::{BindingTable, TypingForm};
use ruff_python_ast::{Expr, ModModule, Stmt};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse Python source into a module AST.
fn parsed(source: &str) -> Result<ModModule, ruff_python_parser::ParseError> {
    Ok(ruff_python_parser::parse_module(source)?.into_syntax())
}

/// The RHS expression of the module-level `target = …` assignment.
fn assigned_value<'a>(body: &'a [Stmt], target: &str) -> Option<&'a Expr> {
    body.iter().find_map(|stmt| {
        let Stmt::Assign(assign) = stmt else {
            return None;
        };
        let is_target = assign
            .targets
            .first()
            .is_some_and(|expr| matches!(expr, Expr::Name(name) if name.id.as_str() == target));
        is_target.then(|| assign.value.as_ref())
    })
}

/// Resolve the RHS of `target = …` through `form_of_with_builtins`.
fn builtin_form_at(source: &str, target: &str) -> Result<Option<TypingForm>, String> {
    let module = parsed(source).map_err(|error| error.to_string())?;
    let table = BindingTable::from_module(&module.body);
    let value = assigned_value(&module.body, target)
        .ok_or_else(|| format!("no assignment to {target} in fixture"))?;
    Ok(table.form_of_with_builtins(value))
}

/// A use BEFORE a later module-level `def staticmethod` still refers to the
/// builtin: the later binding cannot govern an earlier use site.
#[test]
fn builtin_use_before_rebind_still_resolves() -> TestResult {
    let source = "
early = staticmethod

def staticmethod(func):
    return func
";
    assert_eq!(
        builtin_form_at(source, "early")?,
        Some(TypingForm::StaticMethod)
    );
    Ok(())
}

/// A use AFTER the module rebinds the name refers to the rebinding, never
/// the builtin.
#[test]
fn builtin_use_after_rebind_does_not_resolve() -> TestResult {
    let source = "
def staticmethod(func):
    return func

late = staticmethod
";
    assert_eq!(builtin_form_at(source, "late")?, None);
    Ok(())
}

/// A rebinding inside a function body governs that scope only; module-level
/// uses on either side of it still refer to the builtin.
#[test]
fn function_scope_rebind_never_suppresses_module_uses() -> TestResult {
    let source = "
early = staticmethod

def helper():
    staticmethod = None
    return staticmethod

late = staticmethod
";
    assert_eq!(
        builtin_form_at(source, "early")?,
        Some(TypingForm::StaticMethod)
    );
    assert_eq!(
        builtin_form_at(source, "late")?,
        Some(TypingForm::StaticMethod)
    );
    Ok(())
}
