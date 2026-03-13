//! Shared test helpers for basilisk-resolver integration tests.

use basilisk_parser::parse_source;
use basilisk_resolver::{resolve, ResolvedModule};

/// Parse Python source and resolve it in one step.
///
/// Every resolver test needs this exact boilerplate, so centralise it here.
pub fn resolve_src(src: &str) -> Result<ResolvedModule, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(resolved)
}

/// Find a function by name in resolved output, panic with context on miss.
pub fn find_func<'a>(
    resolved: &'a ResolvedModule,
    name: &str,
) -> &'a basilisk_resolver::FunctionInfo {
    resolved
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("function '{name}' not found in resolved output"))
}

/// Find a class by name in resolved output, panic with context on miss.
pub fn find_class<'a>(
    resolved: &'a ResolvedModule,
    name: &str,
) -> &'a basilisk_resolver::ClassInfo {
    resolved
        .classes
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("class '{name}' not found in resolved output"))
}
