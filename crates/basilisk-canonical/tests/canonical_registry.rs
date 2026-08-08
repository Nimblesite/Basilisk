//! Implements [RESOLV-CANONICAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! The registry-load contract: every `[[symbol]]` entry in
//! `resources/typing_symbols.toml` must resolve through [`form_at`].
//!
//! `src/registry.rs` degrades a bad registry (parse failure OR duplicated
//! definition site) to an EMPTY index rather than panicking, and its doc
//! comment names this test as the guard that makes that degradation safe.
//! Without this test a single bad entry fails the whole load and every
//! canonical lookup in the workspace returns `None` — recognition dies while
//! the build stays green.

use std::collections::BTreeMap;

use basilisk_canonical::{form_at, CanonicalSymbol, TypingForm};

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

/// One `[[symbol]]` entry parsed with the SAME typed shape the loader uses, so
/// the declared form is compared, not discarded.
#[derive(serde::Deserialize)]
struct DeclaredSymbol {
    modules: Vec<String>,
    name: String,
    form: TypingForm,
}

/// The registry file's typed top-level shape.
#[derive(serde::Deserialize)]
struct DeclaredRegistry {
    symbol: Vec<DeclaredSymbol>,
}

/// `form_at` must return EXACTLY the form each entry declares. A duplicate
/// `(module, name)` key with a different form makes the loader's last write
/// win silently, so an earlier declaration resolves to the wrong form while
/// every `is_some()` check stays green.
#[test]
fn every_entry_resolves_to_exactly_its_declared_form(
) -> Result<(), Box<dyn std::error::Error>> {
    let declared: DeclaredRegistry = toml::from_str(REGISTRY_SOURCE)?;
    let mut mismatches: Vec<String> = Vec::new();
    for entry in &declared.symbol {
        for module in &entry.modules {
            let resolved = form_at(&CanonicalSymbol::new(module.clone(), entry.name.clone()));
            if resolved != Some(entry.form) {
                mismatches.push(format!(
                    "{module}.{} declared {:?} but resolves to {resolved:?}",
                    entry.name, entry.form
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "registry entries whose declared form is not what form_at returns \
         (conflicting duplicate keys overwrite silently): {mismatches:#?}"
    );
    Ok(())
}

/// Every `(module, name)` definition site must be declared exactly once. Two
/// entries for the same site — even with the same form — make the registry's
/// answer depend on entry order instead of on the data.
#[test]
fn no_definition_site_is_declared_twice() -> Result<(), Box<dyn std::error::Error>> {
    let root: toml::Value = toml::from_str(REGISTRY_SOURCE)?;
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for pair in declared_pairs(&root) {
        *counts.entry(pair).or_insert(0) += 1;
    }
    let duplicated: Vec<String> = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|((module, name), count)| format!("{module}.{name} declared {count} times"))
        .collect();
    assert!(
        duplicated.is_empty(),
        "definition sites declared more than once: {duplicated:#?}"
    );
    Ok(())
}
