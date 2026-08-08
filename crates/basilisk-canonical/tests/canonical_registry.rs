//! Implements [RESOLV-CANONICAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! The registry-load contract: every `[[symbol]]` entry in
//! `resources/typing_symbols.toml` must resolve through [`form_at`].
//!
//! `form.rs` degrades a malformed registry to an EMPTY index rather than
//! panicking, and its doc comment names this test as the guard that makes that
//! degradation safe. Without this test a single bad `form` string fails the
//! whole typed parse and every canonical lookup in the workspace silently
//! returns `None` — recognition dies while the build stays green.

use basilisk_canonical::{form_at, CanonicalSymbol};

/// The same data file `form.rs` embeds; parsed generically here so the test
/// can enumerate entries even when the typed parse is what's broken.
const REGISTRY_SOURCE: &str = include_str!("../resources/typing_symbols.toml");

/// One `(module, name)` pair declared by a `[[symbol]]` entry.
fn declared_pairs(root: &toml::Value) -> Vec<(String, String)> {
    let entries = root
        .get("symbol")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    entries
        .iter()
        .flat_map(|entry| {
            let name = entry.get("name").and_then(toml::Value::as_str);
            let modules = entry
                .get("modules")
                .and_then(toml::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            modules.iter().filter_map(move |module| {
                Some((module.as_str()?.to_owned(), name?.to_owned()))
            })
        })
        .collect()
}

/// A registry entry whose `form` string matches no `TypingForm` variant fails
/// the single typed parse of the whole file, which `form.rs` degrades to an
/// empty index — so EVERY entry stops resolving, not just the bad one.
#[test]
fn every_registry_entry_resolves_to_a_typing_form() -> Result<(), Box<dyn std::error::Error>> {
    let root: toml::Value = toml::from_str(REGISTRY_SOURCE)?;
    let pairs = declared_pairs(&root);
    assert!(
        !pairs.is_empty(),
        "registry data declares no [[symbol]] entries — the data file moved or emptied"
    );

    let unresolved: Vec<String> = pairs
        .iter()
        .filter(|(module, name)| {
            form_at(&CanonicalSymbol::new(module.clone(), name.clone())).is_none()
        })
        .map(|(module, name)| format!("{module}.{name}"))
        .collect();

    assert!(
        unresolved.is_empty(),
        "{} of {} registry entries do not resolve through form_at — \
         a malformed entry has emptied the whole registry: {unresolved:#?}",
        unresolved.len(),
        pairs.len(),
    );
    Ok(())
}
