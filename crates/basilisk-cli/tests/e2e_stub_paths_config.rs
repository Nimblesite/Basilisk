//! Tests for [STUBRES-CONFIG]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CONFIG
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Coarse end-to-end tests for `[tool.basilisk] stub-paths` (issue #173): a
//! local `.pyi` stub placed in a *named* directory (`stubs/`, `typings/`) — the
//! docs-recommended layout — must be discovered, not just stubs dumped in the
//! project root. The reported regression was that only a `stub-paths = ["."]`
//! entry worked; any subdirectory entry was silently ignored because the
//! `pyproject.toml` array was never parsed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory unique to this process and call.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("bsk_stubpaths_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run `basilisk check use.py` from inside `dir` with no ambient venv.
fn check_use_py(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("use.py")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// Lay down the issue #173 repro: a not-installed module whose only typing
/// information is a hand-written `.pyi` stub in `<stub_dir>/`, with
/// `stub-paths = [<stub_dir>]` configured in `pyproject.toml`.
fn write_stub_project(dir: &Path, stub_dir: &str) {
    std::fs::write(
        dir.join("pyproject.toml"),
        format!("[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\nstub-paths = [\"{stub_dir}\"]\n"),
    )
    .expect("write pyproject");
    std::fs::create_dir_all(dir.join(stub_dir)).expect("mkdir stub dir");
    std::fs::write(
        dir.join(stub_dir).join("notinstalledpkg.pyi"),
        "def thing() -> int: ...\n",
    )
    .expect("write stub");
    std::fs::write(
        dir.join("use.py"),
        "from notinstalledpkg import thing\n\nx: int = thing()\n",
    )
    .expect("write use.py");
}

/// Issue #173: a stub under `stubs/` with `stub-paths = ["stubs"]` must resolve
/// the import — no unresolved-import diagnostic, clean run.
#[test]
fn stub_paths_stubs_subdir_resolves_local_stub() {
    let dir = unique_dir("stubs");
    write_stub_project(&dir, "stubs");

    let output = check_use_py(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("imports_unresolved"),
        "stub-paths = [\"stubs\"] must resolve notinstalledpkg from stubs/notinstalledpkg.pyi \
         (no unresolved-import diagnostic), got: {stdout}"
    );
    assert!(
        !stdout.contains("notinstalledpkg"),
        "no diagnostic should mention the now-resolved module, got: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a resolved stub must yield a clean exit, got status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Issue #173: the same must hold for the other docs-recommended directory
/// name, `typings/`.
#[test]
fn stub_paths_typings_subdir_resolves_local_stub() {
    let dir = unique_dir("typings");
    write_stub_project(&dir, "typings");

    let output = check_use_py(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("imports_unresolved"),
        "stub-paths = [\"typings\"] must resolve notinstalledpkg from typings/notinstalledpkg.pyi, \
         got: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a resolved stub must yield a clean exit, got status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Control: with NO `stub-paths` configured, the identical stub sitting in a
/// subdirectory is *not* on the search path, so the import stays unresolved.
/// This proves the two tests above pass because of `stub-paths`, not because
/// the `.pyi` is discovered through some unrelated path.
#[test]
fn without_stub_paths_subdir_stub_is_not_found() {
    let dir = unique_dir("control");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    std::fs::create_dir_all(dir.join("stubs")).expect("mkdir stubs");
    std::fs::write(
        dir.join("stubs/notinstalledpkg.pyi"),
        "def thing() -> int: ...\n",
    )
    .expect("write stub");
    std::fs::write(
        dir.join("use.py"),
        "from notinstalledpkg import thing\n\nx: int = thing()\n",
    )
    .expect("write use.py");

    let output = check_use_py(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("imports_unresolved"),
        "without stub-paths the subdir stub must NOT be searched, so the import is unresolved, \
         got: {stdout}"
    );
}
