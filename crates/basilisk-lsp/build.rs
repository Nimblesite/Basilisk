//! Implements [LSPFMT-PROVENANCE]. See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-PROVENANCE
//!
//! Derives the embedded Ruff formatter version at compile time so it can
//! never drift from the dependency that actually ships.
//!
//! Ruff's workspace crates carry a placeholder version (`0.0.0`) in
//! `Cargo.lock`, so the human-readable release number comes from a declared
//! rev→version pair that this script VERIFIES against the lockfile: if the
//! locked `ruff_python_formatter` rev is not the declared one, the build
//! fails and tells you to update the pair. Bumping the Ruff family without
//! updating the mapping is therefore a compile error, not silent drift.

use std::error::Error;
use std::path::Path;

/// The pinned Ruff rev and the upstream release tag it corresponds to.
/// Must match the `rev` pinned in the workspace `Cargo.toml` (rev 7c645a9 ==
/// tag 0.15.17). The pip-installed `ruff` binary in CI pins the same release.
const PINNED_RUFF_REV: &str = "7c645a9a1be8258b9f9e005208a55a0b7e8e18f0";
const PINNED_RUFF_VERSION: &str = "0.15.17";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let lock_path = Path::new(&manifest_dir).join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let locked_rev = locked_formatter_rev(&lock_path)?;
    if locked_rev != PINNED_RUFF_REV {
        return Err(format!(
            "Cargo.lock pins ruff_python_formatter at rev {locked_rev}, but build.rs \
             declares rev {PINNED_RUFF_REV} = {PINNED_RUFF_VERSION}. Update \
             PINNED_RUFF_REV/PINNED_RUFF_VERSION in crates/basilisk-lsp/build.rs to \
             the new release ([LSPFMT-PROVENANCE])."
        )
        .into());
    }

    println!("cargo:rustc-env=BASILISK_RUFF_FORMATTER_VERSION={PINNED_RUFF_VERSION}");
    Ok(())
}

/// Extract the git rev `ruff_python_formatter` is locked at.
fn locked_formatter_rev(lock_path: &Path) -> Result<String, Box<dyn Error>> {
    let lockfile: toml::Table = std::fs::read_to_string(lock_path)?.parse()?;
    let packages = lockfile
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock has no [[package]] entries")?;
    let source = packages
        .iter()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("ruff_python_formatter"))
        .and_then(|p| p.get("source"))
        .and_then(toml::Value::as_str)
        .ok_or("ruff_python_formatter not found in Cargo.lock")?;
    // Source form: git+https://github.com/astral-sh/ruff?rev=<sha>#<sha>
    let rev = source
        .split_once("rev=")
        .map(|(_, tail)| tail.split('#').next().unwrap_or(tail))
        .ok_or("ruff_python_formatter source has no rev pin")?;
    Ok(rev.to_owned())
}
