//! CLI activation and reporting for [STUBRES-TYPESHED-WARN].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN.

use tracing::{info, warn};

use super::PipelineError;

/// Load import/typeshed configuration and preserve CLI-only target evidence.
///
/// The rule configuration loader also consults `.python-version`; the shared
/// LSP import configuration does not. Copying that detected value before
/// activation keeps stdlib filtering aligned with the checker target while an
/// explicit analysis configuration still wins. [STUBRES-TYPESHED-VERSION]
pub(super) fn load_cli_workspace_config(
    project_root: &std::path::Path,
    detected_python_version: Option<&str>,
) -> basilisk_lsp::config::WorkspaceConfig {
    let mut config = basilisk_lsp::config::load_analysis_config(project_root);
    if config.python_version.is_none() {
        config.python_version = detected_python_version.map(str::to_owned);
    }
    if config.python_platform.is_none() {
        let interpreter = config.python_interpreter.clone().unwrap_or_else(|| {
            std::path::PathBuf::from(basilisk_lsp::debug::resolve_python(project_root))
        });
        config.python_platform = basilisk_uv::python_version::read_python_platform(&interpreter);
    }
    config
}

/// Build the shared CLI/LSP import search path model for a project.
pub(crate) fn build_import_search_paths(
    roots: Vec<std::path::PathBuf>,
    project_root: &std::path::Path,
) -> basilisk_lsp::import_resolver::ImportSearchPaths {
    let config = basilisk_lsp::config::load_analysis_config(project_root);
    build_import_search_paths_with_config(roots, &config)
}

pub(super) fn build_import_search_paths_with_config(
    roots: Vec<std::path::PathBuf>,
    config: &basilisk_lsp::config::WorkspaceConfig,
) -> basilisk_lsp::import_resolver::ImportSearchPaths {
    let registry = build_uv_registry(&roots);
    let mut search_paths =
        basilisk_lsp::import_resolver::search_paths_from_config(&roots, config, registry);
    search_paths.roots = roots;
    info!(
        site_packages = ?search_paths.site_packages,
        has_registry = search_paths.registry.is_some(),
        "built import search paths"
    );
    search_paths
}

pub(super) fn activate_production_typeshed(
    search_paths: &mut basilisk_lsp::import_resolver::ImportSearchPaths,
    config: &basilisk_lsp::config::WorkspaceConfig,
) -> Result<(), PipelineError> {
    let request = basilisk_lsp::config::typeshed_request(config).map_err(PipelineError::Config)?;
    let manager = basilisk_stubs::typeshed::runtime::production_manager(
        request,
        config.typeshed_cache_path.clone(),
    )
    .map_err(|error| PipelineError::Config(error.to_string()))?;
    let snapshot = manager.snapshot().map_err(|error| {
        PipelineError::Internal(format!("typeshed acquisition failed: {error}"))
    })?;
    report_typeshed_status(&snapshot.status);
    search_paths.typeshed_snapshot = Some(basilisk_checker::imports::ActiveTypeshed::new(
        snapshot,
        basilisk_lsp::import_resolver::stub_target_from_config(config),
    ));
    Ok(())
}

fn report_typeshed_status(status: &basilisk_stubs::typeshed::source::TypeshedStatus) {
    let commit_identity = status
        .commit
        .map_or_else(|| "not supplied".to_owned(), |identity| identity.to_hex());
    let tree_identity = status
        .tree
        .map_or_else(|| "not supplied".to_owned(), |identity| identity.to_hex());
    let license_reference = status
        .license_reference
        .as_deref()
        .unwrap_or("not supplied");
    warn!(
        active_source = status.active_source.as_str(),
        commit_identity,
        tree_identity,
        transport = ?status.transport,
        license_status = ?status.license_status,
        license_reference,
        provenance = ?status.provenance,
        signed_release = status.signed_release,
        "typeshed source status"
    );
    for warning in &status.warnings {
        warn!(
            warning_code = warning.code,
            warning_message = warning.message,
            "typeshed source warning"
        );
    }
}

/// Build a uv package registry from workspace roots, if this is a uv project.
fn build_uv_registry(
    roots: &[std::path::PathBuf],
) -> Option<std::sync::Arc<basilisk_uv::PackageRegistry>> {
    let uv_info = basilisk_uv::detect_uv_project(roots)?;

    if !uv_info.has_lockfile {
        info!(
            root = %uv_info.root.display(),
            "uv project detected but no uv.lock — skipping registry"
        );
        return None;
    }

    let lock_path = uv_info.root.join("uv.lock");
    let lock_file = match basilisk_uv::parse_lock_file(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            warn!(
                path = %lock_path.display(),
                %err,
                "failed to parse uv.lock — package registry unavailable"
            );
            return None;
        }
    };

    let deps = basilisk_uv::extract_pyproject_deps(&uv_info.root);
    let registry = basilisk_uv::PackageRegistry::from_lock_file(&lock_file, &deps);

    let pkg_count = registry.all_packages().count();
    info!(
        root = %uv_info.root.display(),
        packages = pkg_count,
        direct_deps = deps.len(),
        "built uv package registry"
    );

    Some(std::sync::Arc::new(registry))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{build_uv_registry, load_cli_workspace_config, report_typeshed_status};

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let mut output = self
                .0
                .lock()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            output.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for Capture {
        type Writer = CaptureWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            CaptureWriter(Arc::clone(&self.0))
        }
    }

    impl Capture {
        fn text(&self) -> Result<String, Box<dyn std::error::Error>> {
            let bytes = self
                .0
                .lock()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .clone();
            Ok(String::from_utf8(bytes)?)
        }
    }

    #[test]
    fn cli_detected_python_version_fills_missing_analysis_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project = tempfile::tempdir()?;
        std::fs::write(project.path().join(".python-version"), "3.12\n")?;
        let detected = basilisk_uv::python_version::resolve_target_python_version(project.path());
        let config = load_cli_workspace_config(project.path(), detected.as_deref());
        assert_eq!(config.python_version.as_deref(), Some("3.12"));
        Ok(())
    }

    #[test]
    fn explicit_analysis_target_wins_over_cli_detected_version(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project = tempfile::tempdir()?;
        std::fs::write(
            project.path().join("pyproject.toml"),
            "[tool.basilisk]\npython-version = \"3.10\"\n",
        )?;
        let config = load_cli_workspace_config(project.path(), Some("3.12"));
        assert_eq!(config.python_version.as_deref(), Some("3.10"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn selected_interpreter_supplies_platform_target_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let project = tempfile::tempdir()?;
        let interpreter = project.path().join("python");
        std::fs::write(&interpreter, "#!/bin/sh\nprintf 'fixture-platform\\n'\n")?;
        std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755))?;
        std::fs::write(
            project.path().join("pyproject.toml"),
            format!("[tool.basilisk]\npython = '{}'\n", interpreter.display()),
        )?;

        let config = load_cli_workspace_config(project.path(), None);

        assert_eq!(
            config.python_platform.as_deref(),
            Some("fixture-platform"),
            "an explicitly selected interpreter is real target evidence for sys.platform"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn explicit_all_platform_keeps_cross_platform_target() -> Result<(), Box<dyn std::error::Error>>
    {
        use std::os::unix::fs::PermissionsExt;

        let project = tempfile::tempdir()?;
        let interpreter = project.path().join("python");
        std::fs::write(&interpreter, "#!/bin/sh\nprintf 'fixture-platform\\n'\n")?;
        std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755))?;
        std::fs::write(
            project.path().join("pyproject.toml"),
            format!(
                "[tool.basilisk]\npython = '{}'\npython-platform = 'All'\n",
                interpreter.display()
            ),
        )?;

        let config = load_cli_workspace_config(project.path(), None);

        assert_eq!(config.python_platform.as_deref(), Some("All"));
        Ok(())
    }

    #[test]
    fn uv_detection_without_a_lockfile_does_not_build_a_registry(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project = tempfile::tempdir()?;
        std::fs::write(project.path().join(".python-version"), "3.13\n")?;

        assert!(build_uv_registry(&[project.path().to_path_buf()]).is_none());
        Ok(())
    }

    #[test]
    fn malformed_uv_lockfile_does_not_build_a_partial_registry(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project = tempfile::tempdir()?;
        std::fs::write(project.path().join("uv.lock"), "not valid TOML = [")?;

        assert!(build_uv_registry(&[project.path().to_path_buf()]).is_none());
        Ok(())
    }

    #[test]
    fn valid_uv_lockfile_builds_the_registry() -> Result<(), Box<dyn std::error::Error>> {
        let project = tempfile::tempdir()?;
        std::fs::write(
            project.path().join("uv.lock"),
            "version = 1\n\n[[package]]\nname = 'example'\nversion = '1.0.0'\n",
        )?;

        let registry = build_uv_registry(&[project.path().to_path_buf()])
            .ok_or("valid uv.lock should build a registry")?;
        assert_eq!(registry.all_packages().count(), 1);
        Ok(())
    }

    #[test]
    fn latest_fallback_status_is_loud_and_ordered() -> Result<(), Box<dyn std::error::Error>> {
        use basilisk_stubs::typeshed::source::StatusWarning;
        use basilisk_stubs::typeshed::warning::{TypeshedWarning, UnpinnedKind};

        let mut status = basilisk_stubs::typeshed::bundle::bundled_snapshot()?.status;
        status.warnings = StatusWarning::list(&[
            TypeshedWarning::DownloadFailed {
                bundled_sha: status
                    .commit
                    .ok_or("bundled status is missing its commit identity")?
                    .to_hex(),
            },
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
            TypeshedWarning::Unverified,
        ]);
        let capture = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(capture.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || report_typeshed_status(&status));

        let stderr = capture.text()?;
        assert!(stderr.contains("typeshed source status"), "{stderr}");
        assert!(stderr.contains("signed_release=false"), "{stderr}");
        let unpinned = stderr.find("warning_code=\"UNPINNED\"");
        let failed = stderr.find("warning_code=\"DOWNLOAD FAILED\"");
        let unverified = stderr.find("warning_code=\"UNVERIFIED\"");
        assert!(
            unpinned
                .zip(failed)
                .zip(unverified)
                .is_some_and(|((first, second), third)| first < second && second < third),
            "Latest fallback warnings must remain loud and canonical: {stderr}"
        );
        Ok(())
    }

    #[test]
    fn status_reporting_names_missing_optional_identities() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut status = basilisk_stubs::typeshed::bundle::bundled_snapshot()?.status;
        status.commit = None;
        status.tree = None;
        status.license_reference = None;
        status.warnings.clear();
        let capture = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(capture.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || report_typeshed_status(&status));

        let stderr = capture.text()?;
        for field in [
            "commit_identity=\"not supplied\"",
            "tree_identity=\"not supplied\"",
            "license_reference=\"not supplied\"",
        ] {
            assert!(stderr.contains(field), "missing `{field}` in: {stderr}");
        }
        Ok(())
    }
}
