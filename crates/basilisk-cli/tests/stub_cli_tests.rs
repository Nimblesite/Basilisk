//! Binary coverage for [STUBRES-AUTOGEN].
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let sequence = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "basilisk_stub_cli_{name}_{}_{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        std::fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"stub-cli-test\"\nversion = \"0.0.0\"\n",
        )?;
        Ok(Self { root })
    }

    fn write(&self, relative: &str, source: &str) -> Result<PathBuf, std::io::Error> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, source)?;
        Ok(path)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_basilisk"));
        let _ = command.current_dir(&self.root);
        command
    }

    fn generate_all(&self) -> Result<Output, std::io::Error> {
        self.command()
            .args(["stubs", "generate", "--all", "--mode", "ast"])
            .output()
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn site_packages(project: &TestProject) -> PathBuf {
    project.root.join(".venv/lib/python3.12/site-packages")
}

// Tests [STUBRES-AUTOGEN]: `stubs generate --all` discovers every untyped
// third-party import, deduplicates it, and ignores local or PEP 561-typed code.
#[test]
fn generate_all_discovers_only_untyped_project_imports() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("all")?;
    let packages = site_packages(&project);
    let _ = project.write(
        ".venv/lib/python3.12/site-packages/alpha.py",
        "def alpha_value(value: int) -> str:\n    return str(value)\n",
    )?;
    let _ = project.write(
        ".venv/lib/python3.12/site-packages/beta/__init__.py",
        "def beta_value() -> int:\n    return 2\n",
    )?;
    let _ = project.write(
        ".venv/lib/python3.12/site-packages/typedpkg/__init__.py",
        "def typed_value() -> int:\n    return 3\n",
    )?;
    let _ = project.write(".venv/lib/python3.12/site-packages/typedpkg/py.typed", "")?;
    let _ = project.write("localmod.py", "def local_value() -> int:\n    return 4\n")?;
    let _ = project.write(
        "app.py",
        "import alpha\nfrom beta import beta_value\nimport typedpkg\nimport localmod\n",
    )?;
    let _ = project.write("worker.py", "import alpha\n")?;

    assert!(packages.is_dir(), "test venv must expose site-packages");
    let output = project.generate_all()?;
    let details = output_text(&output);

    assert!(output.status.success(), "{details}");
    let alpha = project.root.join(".basilisk/stubs/alpha.pyi");
    let beta = project.root.join(".basilisk/stubs/beta.pyi");
    assert!(alpha.is_file(), "alpha stub missing; {details}");
    assert!(beta.is_file(), "beta stub missing; {details}");
    assert!(
        std::fs::read_to_string(alpha)?.contains("def alpha_value(value: int) -> str: ..."),
        "alpha stub must come from its source"
    );
    assert!(
        std::fs::read_to_string(beta)?.contains("def beta_value() -> int: ..."),
        "beta stub must come from its package source"
    );
    assert!(
        !project.root.join(".basilisk/stubs/typedpkg.pyi").exists(),
        "a py.typed package must not be regenerated"
    );
    assert!(
        !project.root.join(".basilisk/stubs/localmod.pyi").exists(),
        "first-party source must not be treated as an untyped dependency"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("Generated stub for `alpha`")
            .count(),
        1,
        "duplicate imports must generate one stub; {details}"
    );
    Ok(())
}

// Tests [STUBRES-AUTOGEN]: Pyright's top-level `--createstub` spelling is an
// alias of the named-package `stubs generate` workflow, including mode flags.
#[test]
fn createstub_alias_generates_the_named_package() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("createstub")?;
    let modules = project.root.join("python-modules");
    let _ = project.write(
        "python-modules/aliaspkg.py",
        "def aliased(value: str) -> int:\n    return len(value)\n",
    )?;

    let output = project
        .command()
        .env("PYTHONPATH", &modules)
        .args(["--createstub", "aliaspkg", "--mode", "ast"])
        .output()?;
    let details = output_text(&output);

    assert!(output.status.success(), "{details}");
    let stub = project.root.join(".basilisk/stubs/aliaspkg.pyi");
    assert!(
        stub.is_file(),
        "compatibility alias must write the stub; {details}"
    );
    assert!(
        std::fs::read_to_string(stub)?.contains("def aliased(value: str) -> int: ..."),
        "compatibility alias must use the normal generation backend"
    );
    Ok(())
}

#[test]
fn generate_all_with_no_untyped_imports_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("all_empty")?;
    let _ = project.write("app.py", "import pathlib\n")?;

    let output = project.generate_all()?;
    let details = output_text(&output);

    assert!(output.status.success(), "{details}");
    assert!(
        !project.root.join(".basilisk/stubs").exists(),
        "an empty discovery should not create a cache directory"
    );
    Ok(())
}
