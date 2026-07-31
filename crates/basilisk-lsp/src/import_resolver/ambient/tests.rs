use super::*;

fn fake_python(prefix: &Path, version: &str) -> std::io::Result<PathBuf> {
    fake_named_python(prefix, "python3", version)
}

/// A conventional `<prefix>/bin/<name>` + `<prefix>/lib/python<version>/
/// site-packages` layout. The binary is an EMPTY file on purpose: spawning it
/// can never answer, so any resolved `site-packages` proves the direct
/// filesystem path was used.
fn fake_named_python(prefix: &Path, name: &str, version: &str) -> std::io::Result<PathBuf> {
    let bin = prefix.join("bin");
    std::fs::create_dir_all(&bin)?;
    let executable = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&executable, [])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::create_dir_all(prefix.join(format!("lib/python{version}/site-packages")))?;
    Ok(executable)
}

/// A bare command name (`python3.12`) locates its binary on the injected
/// search path and resolves the prefix-encoded `site-packages` directly — no
/// interpreter launch on the per-check path ([ANALYSIS-CROSSLSP-IMPORT]).
#[test]
fn bare_versioned_name_resolves_conventional_prefix_without_spawning(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("prefix");
    let executable = fake_named_python(&prefix, "python3.12", "3.12")?;
    let search_path = std::env::join_paths([executable.parent().ok_or("missing bin")?])?;

    assert_eq!(
        detect_for_interpreter_on(Path::new("python3.12"), Some("3.12"), Some(&search_path)),
        Some(prefix.join("lib/python3.12/site-packages"))
    );
    Ok(())
}

/// A venv `python` is a symlink to the base interpreter; the venv's OWN
/// `site-packages` must win, so the raw path is inspected before its
/// canonicalized form ([ANALYSIS-CROSSLSP-IMPORT]).
#[cfg(unix)]
#[test]
fn venv_symlink_resolves_the_venv_site_packages_not_the_base(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let base = root.path().join("base");
    let base_python = fake_python(&base, "3.12")?;
    let venv = root.path().join("venv");
    let venv_bin = venv.join("bin");
    std::fs::create_dir_all(&venv_bin)?;
    std::fs::create_dir_all(venv.join("lib/python3.12/site-packages"))?;
    std::os::unix::fs::symlink(&base_python, venv_bin.join("python"))?;

    assert_eq!(
        detect_for_interpreter(&venv_bin.join("python"), Some("3.12")),
        Some(venv.join("lib/python3.12/site-packages"))
    );
    Ok(())
}

/// An explicit path is never re-resolved against the search path; only bare
/// command names are located.
#[test]
fn locate_bare_name_ignores_explicit_paths() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let executable = fake_python(&root.path().join("prefix"), "3.12")?;
    let search_path = std::env::join_paths([executable.parent().ok_or("missing bin")?])?;

    assert_eq!(
        locate_bare_name(Path::new("python3"), Some(&search_path)).as_ref(),
        Some(&executable)
    );
    assert_eq!(locate_bare_name(&executable, Some(&search_path)), None);
    Ok(())
}

#[test]
fn parses_exact_python_versions_only() {
    assert_eq!(
        parse_python_version_component("python3.12"),
        Some("3.12".to_owned())
    );
    assert_eq!(
        parse_python_version_component("Python312"),
        Some("3.12".to_owned())
    );
    assert_eq!(
        parse_python_version_component("python3.14.exe"),
        Some("3.14".to_owned())
    );
    assert_eq!(parse_python_version_component("python3"), None);
}

/// [TYPESHEDRT-ACCEPTANCE-TARGET] explicit Python-binary override: an
/// explicitly configured interpreter resolves against *its own*
/// `site-packages` from the prefix layout, with no process spawn — the
/// deterministic cross-version case.
#[test]
fn detect_for_interpreter_resolves_explicit_binary_site_packages(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("venv");
    // Creates <prefix>/bin/python3 + <prefix>/lib/python3.11/site-packages.
    let interpreter = fake_python(&prefix, "3.11")?;
    let expected = prefix.join("lib/python3.11/site-packages");
    assert_eq!(detect_for_interpreter(&interpreter, None), Some(expected));
    Ok(())
}

/// A different interpreter version selects that version's `site-packages`,
/// proving cross-version resolution follows the explicit binary
/// ([TYPESHEDRT-ACCEPTANCE-TARGET]).
#[test]
fn detect_for_interpreter_is_version_specific() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("venv");
    let interpreter = fake_python(&prefix, "3.9")?;
    assert_eq!(
        detect_for_interpreter(&interpreter, Some("3.9")),
        Some(prefix.join("lib/python3.9/site-packages"))
    );
    Ok(())
}

/// An interpreter with neither a prefix `site-packages` layout nor a
/// runnable binary yields `None` rather than a manufactured path
/// ([TYPESHEDRT-ACCEPTANCE-TARGET]).
#[test]
fn detect_for_interpreter_without_layout_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin)?;
    let interpreter = bin.join(format!("python3{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&interpreter, [])?;
    assert_eq!(detect_for_interpreter(&interpreter, None), None);
    Ok(())
}

#[test]
fn direct_discovery_defers_without_a_user_site() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("python3.12");
    let executable = fake_python(&prefix, "3.12")?;
    let search_path = std::env::join_paths([executable.parent().ok_or("missing bin")?])?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(search_path),
        ..Default::default()
    };

    assert_eq!(detect_direct(&environment), None);
    Ok(())
}

#[test]
fn direct_discovery_prefers_user_base() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("python3.12");
    let executable = fake_python(&prefix, "3.12")?;
    let user_base = root.path().join("user");
    let user_site = if cfg!(target_os = "macos") {
        user_base.join("lib/python/site-packages")
    } else if cfg!(windows) {
        user_base.join("Python312/site-packages")
    } else {
        user_base.join("lib/python3.12/site-packages")
    };
    std::fs::create_dir_all(&user_site)?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(std::env::join_paths([executable
            .parent()
            .ok_or("missing bin")?])?),
        user_base: Some(user_base),
        ..Default::default()
    };

    assert_eq!(detect_direct(&environment), Some(user_site));
    Ok(())
}

#[test]
fn python_path_site_packages_has_priority() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let site_packages = root.path().join("site-packages");
    std::fs::create_dir_all(&site_packages)?;
    let executable = fake_python(&root.path().join("python3.12"), "3.12")?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(std::env::join_paths([executable
            .parent()
            .ok_or("missing bin")?])?),
        python_path: Some(std::env::join_paths([&site_packages])?),
        ..Default::default()
    };

    assert_eq!(
        detect_direct(&environment),
        Some(std::fs::canonicalize(site_packages)?)
    );
    Ok(())
}

#[test]
fn no_user_site_flag_defers_to_interpreter() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("python3.12");
    let executable = fake_python(&prefix, "3.12")?;
    let user_base = root.path().join("user");
    std::fs::create_dir_all(user_base.join("lib/python3.12/site-packages"))?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(std::env::join_paths([executable
            .parent()
            .ok_or("missing bin")?])?),
        user_base: Some(user_base),
        no_user_site: Some("1".into()),
        ..Default::default()
    };

    assert_eq!(detect_direct(&environment), None);
    Ok(())
}

#[test]
fn python_home_defers_even_with_python_path() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let site_packages = root.path().join("site-packages");
    std::fs::create_dir_all(&site_packages)?;
    let executable = fake_python(&root.path().join("python3.12"), "3.12")?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(std::env::join_paths([executable
            .parent()
            .ok_or("missing bin")?])?),
        python_path: Some(std::env::join_paths([site_packages])?),
        python_home: Some("/custom/python".into()),
        ..Default::default()
    };

    assert_eq!(detect_direct(&environment), None);
    Ok(())
}

#[test]
fn zero_no_user_site_flag_keeps_user_site_enabled() {
    assert!(!python_env_flag_enabled(std::ffi::OsStr::new("0")));
    assert!(!python_env_flag_enabled(std::ffi::OsStr::new("00")));
    assert!(python_env_flag_enabled(std::ffi::OsStr::new("1")));
    assert!(python_env_flag_enabled(std::ffi::OsStr::new("yes")));
    assert!(python_env_flag_enabled(std::ffi::OsStr::new(" 0 ")));
    assert!(!python_env_flag_enabled(std::ffi::OsStr::new("+0")));
    assert!(!python_env_flag_enabled(std::ffi::OsStr::new("-0")));
}

#[test]
#[cfg(unix)]
fn path_lookup_rejects_non_executable_python() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let executable = root.path().join("python3");
    std::fs::write(executable, [])?;
    let environment = AmbientPythonEnvironment {
        search_path: Some(std::env::join_paths([root.path()])?),
        ..Default::default()
    };

    assert_eq!(find_python_executable(&environment), None);
    Ok(())
}
