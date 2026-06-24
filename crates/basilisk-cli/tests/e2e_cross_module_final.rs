//! Tests for [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    unused_results,
    dead_code
)]
//! E2E: overriding a `@final` method whose definition lives in an imported
//! sibling `.pyi` stub must be flagged (BSK-E0034 cross-module final override).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_checker::check;
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

static CTR: AtomicU64 = AtomicU64::new(0);

fn unique_tmp(prefix: &str) -> PathBuf {
    let ctr = CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{ctr}_{}", std::process::id()))
}

fn codes_for(main: &Path) -> Vec<String> {
    let parsed = parse_file(main.to_str().unwrap()).unwrap();
    let resolved = resolve(&parsed).unwrap();
    check(&resolved)
        .iter()
        .map(|d| d.code.code.to_owned())
        .collect()
}

#[test]
fn cross_module_final_first_overload_override_fires() {
    let dir = unique_tmp("e2e_xmod_final_a");
    fs::create_dir_all(&dir).unwrap();
    // `@final` on the first overload of a stub marks the whole method final.
    fs::write(
        dir.join("_basemod.pyi"),
        "from typing import final, overload\n\
         class Base:\n\
         \x20   @final\n\
         \x20   @overload\n\
         \x20   def method(self, x: int) -> int: ...\n\
         \x20   @overload\n\
         \x20   def method(self, x: str) -> str: ...\n",
    )
    .unwrap();
    let main = dir.join("main.py");
    fs::write(
        &main,
        "from _basemod import Base\n\
         class D(Base):\n\
         \x20   def method(self, x):\n\
         \x20       return x\n",
    )
    .unwrap();

    let codes = codes_for(&main);
    assert!(
        codes.contains(&"BSK-E0034".to_owned()),
        "overriding an imported @final method must fire E0034, got: {codes:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cross_module_final_swapped_decorator_order_fires() {
    let dir = unique_tmp("e2e_xmod_final_b");
    fs::create_dir_all(&dir).unwrap();
    // `@overload` then `@final` (swapped) on the first overload is equivalent.
    fs::write(
        dir.join("_basemod.pyi"),
        "from typing import final, overload\n\
         class Base:\n\
         \x20   @overload\n\
         \x20   @final\n\
         \x20   def method(self, x: int) -> int: ...\n\
         \x20   @overload\n\
         \x20   def method(self, x: str) -> str: ...\n",
    )
    .unwrap();
    let main = dir.join("main.py");
    fs::write(
        &main,
        "from _basemod import Base\n\
         class D(Base):\n\
         \x20   def method(self, x):\n\
         \x20       return x\n",
    )
    .unwrap();

    let codes = codes_for(&main);
    assert!(
        codes.contains(&"BSK-E0034".to_owned()),
        "swapped @overload/@final order must still fire E0034, got: {codes:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cross_module_non_final_override_ok() {
    let dir = unique_tmp("e2e_xmod_final_c");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("_basemod.pyi"),
        "class Base:\n\x20   def method(self, x: int) -> int: ...\n",
    )
    .unwrap();
    let main = dir.join("main.py");
    fs::write(
        &main,
        "from _basemod import Base\n\
         class D(Base):\n\
         \x20   def method(self, x):\n\
         \x20       return x\n",
    )
    .unwrap();

    let codes = codes_for(&main);
    assert!(
        !codes.contains(&"BSK-E0034".to_owned()),
        "overriding a non-final imported method must not fire E0034, got: {codes:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}
