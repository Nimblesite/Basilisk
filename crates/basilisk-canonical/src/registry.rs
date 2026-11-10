//! Implements [RESOLV-CANONICAL-REGISTRY].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-REGISTRY
//!
//! The registry loader. Parses `resources/typing_symbols.toml` exactly once
//! into the module → name → form index the lookup API in [`crate::form`]
//! reads.
//!
//! The load is all-or-nothing: a malformed entry or a duplicated definition
//! site rejects the WHOLE file, loudly, rather than letting entry order or a
//! partial parse decide what a symbol means.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::TypingForm;

/// One registry entry as it appears in the data file.
#[derive(Debug, Deserialize)]
struct RegistryEntry {
    modules: Vec<String>,
    name: String,
    form: TypingForm,
}

/// The registry data file's top-level shape.
#[derive(Debug, Deserialize)]
struct RegistryFile {
    symbol: Vec<RegistryEntry>,
}

/// The specification registry, as data. No Rust file contains these spellings.
const REGISTRY_SOURCE: &str = include_str!("../resources/typing_symbols.toml");

/// Module → name → form, built once from the registry data file.
pub(crate) type RegistryIndex = HashMap<String, HashMap<String, TypingForm>>;

/// Why the registry data file failed to load.
#[derive(Debug, thiserror::Error)]
enum RegistryLoadError {
    /// The typed TOML parse failed. One malformed `form` string fails the
    /// whole file, never just its own entry.
    #[error("registry data does not parse: {0}")]
    Parse(#[from] toml::de::Error),
    /// A `(module, name)` definition site is declared more than once. Which
    /// declaration won would depend on entry order, so the file is rejected
    /// whole.
    #[error("definition site {module}.{name} is declared more than once")]
    DuplicateSite {
        /// Module of the duplicated definition site.
        module: String,
        /// Name of the duplicated definition site.
        name: String,
    },
}

/// Insert one definition site, rejecting any duplicate declaration.
fn insert_site(
    index: &mut RegistryIndex,
    module: &str,
    name: &str,
    form: TypingForm,
) -> Result<(), RegistryLoadError> {
    let previous = index
        .entry(module.to_owned())
        .or_default()
        .insert(name.to_owned(), form);
    match previous {
        None => Ok(()),
        Some(_) => Err(RegistryLoadError::DuplicateSite {
            module: module.to_owned(),
            name: name.to_owned(),
        }),
    }
}

/// Parse registry data into an index, rejecting duplicate definition sites.
fn build_index(source: &str) -> Result<RegistryIndex, RegistryLoadError> {
    let parsed = toml::from_str::<RegistryFile>(source)?;
    let mut index = RegistryIndex::new();
    for entry in &parsed.symbol {
        for module in &entry.modules {
            insert_site(&mut index, module, &entry.name, entry.form)?;
        }
    }
    Ok(index)
}

/// The parsed registry, or an empty index if the data file fails to load.
///
/// A bad registry is a build-time defect pinned by
/// `tests/canonical_registry.rs`; at runtime the failure is reported loudly
/// and every canonical lookup answers `None`, because panicking in a library
/// the LSP links would take the whole server down with it.
pub(crate) fn registry() -> &'static RegistryIndex {
    static REGISTRY: OnceLock<RegistryIndex> = OnceLock::new();
    REGISTRY.get_or_init(|| match build_index(REGISTRY_SOURCE) {
        Ok(index) => index,
        Err(error) => {
            tracing::error!(
                %error,
                "canonical registry failed to load; every canonical lookup will answer None"
            );
            RegistryIndex::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same site declared twice with CONFLICTING forms: rejected, naming the
    /// site, instead of the last declaration silently winning.
    #[test]
    fn conflicting_duplicate_site_is_rejected() -> Result<(), String> {
        let source = r#"
[[symbol]]
modules = ["m"]
name = "S"
form = "abstract-set"

[[symbol]]
modules = ["m"]
name = "S"
form = "set-alias"
"#;
        match build_index(source) {
            Err(RegistryLoadError::DuplicateSite { module, name }) => {
                assert_eq!(module, "m");
                assert_eq!(name, "S");
                Ok(())
            }
            other => Err(format!("expected DuplicateSite, got {other:?}")),
        }
    }

    /// Same site declared twice with the SAME form: still rejected — which
    /// copy survived would depend on entry order, and the duplicate is dead
    /// data either way.
    #[test]
    fn same_form_duplicate_site_is_rejected() -> Result<(), String> {
        let source = r#"
[[symbol]]
modules = ["m"]
name = "S"
form = "set-alias"

[[symbol]]
modules = ["m"]
name = "S"
form = "set-alias"
"#;
        match build_index(source) {
            Err(RegistryLoadError::DuplicateSite { .. }) => Ok(()),
            other => Err(format!("expected DuplicateSite, got {other:?}")),
        }
    }

    /// A `form` string naming no `TypingForm` variant fails the typed parse.
    #[test]
    fn unknown_form_string_fails_the_parse() -> Result<(), String> {
        let source = r#"
[[symbol]]
modules = ["m"]
name = "S"
form = "no-such-form"
"#;
        match build_index(source) {
            Err(RegistryLoadError::Parse(_)) => Ok(()),
            other => Err(format!("expected Parse, got {other:?}")),
        }
    }

    /// A valid file indexes every (module, name) pair to its declared form.
    #[test]
    fn valid_registry_indexes_every_declared_site() -> Result<(), RegistryLoadError> {
        let source = r#"
[[symbol]]
modules = ["m", "n"]
name = "S"
form = "class-var"
"#;
        let index = build_index(source)?;
        for module in ["m", "n"] {
            let form = index.get(module).and_then(|names| names.get("S")).copied();
            assert_eq!(form, Some(TypingForm::ClassVar), "module {module}");
        }
        Ok(())
    }

    /// The embedded data file itself loads: no parse failure, no duplicates.
    #[test]
    fn embedded_registry_loads() -> Result<(), String> {
        match build_index(REGISTRY_SOURCE) {
            Ok(index) => {
                assert!(!index.is_empty(), "embedded registry indexed no modules");
                Ok(())
            }
            Err(error) => Err(format!("embedded registry failed to load: {error}")),
        }
    }
}
