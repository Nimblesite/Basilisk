//! Shared test helpers for basilisk-resolver integration tests.

use basilisk_parser::parse_source;
use basilisk_resolver::{resolve, ResolvedModule};

/// Parse Python source and resolve it in one step.
pub fn resolve_src(src: &str) -> Result<ResolvedModule, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(resolved)
}
