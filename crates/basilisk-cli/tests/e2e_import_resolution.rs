//! Tests for [LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND], [LSPUV-LOCK-IMPORT-MAPPING],
//! and [LSPUV-WORKSPACE-IMPORT-RESOLUTION]. See docs/specs/LSP-UV-SPEC.md.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Coarse end-to-end tests for import-resolution classification (issue #25)
//! and src-layout first-party resolution in the CLI (issue #24).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory unique to this process and call.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_imres_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run `basilisk check <args...>` from inside `dir` with no ambient venv.
fn check(dir: &Path, args: &[&str]) -> Output {
    check_with_venv(dir, args, None)
}

/// Like [`check`], but points `VIRTUAL_ENV` at `venv` when `Some` (the standard
/// signal for an active environment) and removes it otherwise. The `Some` form
/// pins import resolution to a hermetic env so a test never falls back to the
/// host interpreter's global site-packages (where e.g. Pillow may be installed
/// and would mask the diagnostic under test).
fn check_with_venv(dir: &Path, args: &[&str], venv: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_basilisk"));
    let _ = cmd.arg("check").args(args).current_dir(dir);
    match venv {
        Some(path) => {
            let _ = cmd.env("VIRTUAL_ENV", path);
        }
        None => {
            let _ = cmd.env_remove("VIRTUAL_ENV");
        }
    }
    cmd.output().expect("spawn basilisk")
}

/// Issue #25: an unsynced-but-declared dependency whose import name differs
/// from its distribution name (Pillow → PIL) must be classified as
/// "declared but the environment is not synced", not "not a dependency".
#[test]
fn declared_unsynced_pillow_classified_as_needs_sync() {
    let dir = unique_dir("pillow");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\ndependencies = [\"pillow>=11.0.0\"]\n",
    )
    .expect("write pyproject");
    std::fs::write(
        dir.join("uv.lock"),
        "version = 1\nrequires-python = \">=3.12\"\n\n[[package]]\nname = \"pillow\"\nversion = \"11.0.0\"\n",
    )
    .expect("write lock");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
    std::fs::write(dir.join("src/app.py"), "from PIL import Image\n").expect("write app");

    // Hermetic empty environment: a venv with NO packages installed models
    // "declared but not synced" exactly, and pinning `VIRTUAL_ENV` to it stops
    // the resolver falling back to the host interpreter (which may have Pillow
    // installed globally and would otherwise resolve PIL and mask E0010).
    let venv = dir.join(".venv");
    std::fs::create_dir_all(venv.join("lib/python3.12/site-packages")).expect("venv lib");
    std::fs::create_dir_all(venv.join("Lib/site-packages")).expect("venv Lib");

    let output = check_with_venv(&dir, &["src"], Some(&venv));
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("not a dependency in pyproject.toml"),
        "PIL is declared (as pillow) — must not be classified NotInstalled, got: {stdout}"
    );
    assert!(
        stdout.contains("declared but the environment is not synced"),
        "PIL should be classified as needs-sync, got: {stdout}"
    );
}

/// Issue #24: in a src-layout project, both `tests.helpers` and the src
/// package must resolve when checking from the project root.
#[test]
fn src_layout_first_party_imports_resolve() {
    let dir = unique_dir("srclayout");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"agent-backend\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    std::fs::create_dir_all(dir.join("src/agent_backend/db")).expect("mkdir pkg");
    std::fs::create_dir_all(dir.join("tests")).expect("mkdir tests");
    std::fs::write(dir.join("src/agent_backend/__init__.py"), "").expect("write init");
    std::fs::write(dir.join("src/agent_backend/db/__init__.py"), "").expect("write db init");
    std::fs::write(
        dir.join("src/agent_backend/db/models.py"),
        "class AgentConfig:\n    pass\n",
    )
    .expect("write models");
    std::fs::write(
        dir.join("tests/helpers.py"),
        "from agent_backend.db.models import AgentConfig\n",
    )
    .expect("write helpers");
    std::fs::write(
        dir.join("tests/test_foo.py"),
        "from tests.helpers import AgentConfig\n",
    )
    .expect("write test_foo");

    let output = check(&dir, &["."]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("imports_unresolved"),
        "first-party src-layout imports must resolve, got: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "src-layout project must check clean, stdout: {stdout}"
    );
}

/// Issue #22: a bare `import foo` where `foo.py` is a sibling file in the
/// same scripts directory (no `__init__.py`) must resolve via the importing
/// file's own directory (sys.path[0] semantics) — even when the check is
/// pointed at the project ROOT, not the scripts directory.
#[test]
fn sibling_script_import_resolves_from_project_root() {
    let dir = unique_dir("sibling");
    std::fs::create_dir_all(dir.join("scripts")).expect("mkdir scripts");
    std::fs::write(
        dir.join("scripts/configure_agent_backend.py"),
        "def main() -> None:\n    pass\n",
    )
    .expect("write sibling");
    std::fs::write(
        dir.join("scripts/configure_agent_backend_test.py"),
        "from configure_agent_backend import main\nimport configure_agent_backend as subject\n",
    )
    .expect("write importer");

    let output = check(&dir, &["."]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("imports_unresolved"),
        "sibling-module script imports must resolve (issue #22), got: {stdout}"
    );
}

/// Issue #13: packages shipping a PEP 561 `py.typed` marker are typed — no
/// stub diagnostic; a genuinely untyped package still fires, and its help must
/// not fabricate a nonexistent `types-X` distribution.
#[test]
fn py_typed_packages_not_flagged_untyped_ones_still_fire() {
    let dir = unique_dir("pytyped");
    let site = dir.join(".venv/lib/python3.12/site-packages");
    std::fs::create_dir_all(site.join("typedpkg_fake/orm")).expect("mkdir typed");
    std::fs::create_dir_all(site.join("untypedpkg_fake")).expect("mkdir untyped");
    std::fs::write(site.join("typedpkg_fake/__init__.py"), "").expect("write init");
    std::fs::write(site.join("typedpkg_fake/py.typed"), "").expect("write marker");
    std::fs::write(
        site.join("typedpkg_fake/orm/__init__.py"),
        "class Session:\n    pass\n",
    )
    .expect("write orm");
    std::fs::write(site.join("untypedpkg_fake/__init__.py"), "").expect("write untyped");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
    std::fs::write(
        dir.join("src/app.py"),
        "import typedpkg_fake\nfrom typedpkg_fake.orm import Session\nimport untypedpkg_fake\n",
    )
    .expect("write app");

    let output = check(&dir, &["src"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("`typedpkg_fake`"),
        "a py.typed package must not be flagged as untyped (issue #13), got: {stdout}"
    );
    assert!(
        stdout.contains("`untypedpkg_fake`"),
        "a genuinely untyped package must still be flagged, got: {stdout}"
    );
    assert!(
        !stdout.contains("types-untypedpkg_fake"),
        "help text must not fabricate a nonexistent types-X distribution, got: {stdout}"
    );
}
