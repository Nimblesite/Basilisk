//! Real bundled-Typeshed acceptance for [STUBRES-PYI] #289.
//!
//! This deliberately enters through the active [`Snapshot`] and Salsa
//! cross-module query. No test-built `ExternalSymbol` or hand-written Mock
//! hierarchy is allowed in this regression.

#![allow(
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    reason = "end-to-end acceptance fixture"
)]

use std::sync::Arc;

use basilisk_checker::imports::{ActiveTypeshed, ImportSearchPaths};
use basilisk_checker::{
    cross_resolved_module, BasiliskDatabase, FileRegistry, ResolvedFile, SearchPathsInput,
    SourceFile, WorkspaceFiles,
};
use basilisk_lsp::hover::hover_at;
use basilisk_stubs::types::{StubTarget, StubTargetPlatform};
use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
use basilisk_stubs::typeshed::gittree::FileMode;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::{
    LicenseStatus, Provenance, SourceIdentity, SourceKind, Transport, TypeshedStatus,
};
use tower_lsp::lsp_types::HoverContents;

fn resolve_with_snapshot(
    snapshot: Arc<Snapshot>,
    source: &str,
) -> Arc<basilisk_resolver::ResolvedModule> {
    let search_paths = ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(
            snapshot,
            Some(StubTarget {
                python_version: (3, 12),
                platform: StubTargetPlatform::Concrete("linux".to_owned()),
            }),
        )),
    };
    let database = BasiliskDatabase::default();
    let search_input = SearchPathsInput::new(&database, search_paths);
    let workspace = WorkspaceFiles::new(&database, FileRegistry::default());
    let file = SourceFile::new(&database, "main.py".to_owned(), source.to_owned());
    let ResolvedFile::Resolved(resolved) =
        cross_resolved_module(&database, file, search_input, workspace)
    else {
        panic!("the Mock fixture must parse and resolve");
    };
    Arc::clone(resolved)
}

fn custom_mock_snapshot() -> Arc<Snapshot> {
    let identity = SourceIdentity::Custom {
        digest: "mock-override".to_owned(),
    };
    let archive = Archive::new(vec![
        ArchiveEntry {
            path: "stdlib/VERSIONS".to_owned(),
            mode: FileMode::Regular,
            data: b"unittest: 3.8-\nunittest.mock: 3.8-\n".to_vec(),
        },
        ArchiveEntry {
            path: "stdlib/unittest/mock.pyi".to_owned(),
            mode: FileMode::Regular,
            data: b"class Mock:\n    def __init__(self, custom_token: bytes, /) -> None: ...\n"
                .to_vec(),
        },
    ]);
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        transport: Transport::CustomPath,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        provenance: Provenance::UserManaged,
        signed_release: false,
        warnings: Vec::new(),
    };
    Arc::new(
        Snapshot::build(
            identity.clone(),
            status,
            ArchiveVfs::new(identity.uri_component(), archive),
            None,
        )
        .expect("the custom Mock snapshot must activate"),
    )
}

/// [STUBRES-PYI] #289 and [STUBRES-TYPESHED-BASELINE]: the offline bundled
/// ZIP supplies the real `unittest.mock.Mock` declaration. Resolution parses
/// that exact VFS body, follows the C3 hierarchy, binds `cls`/`self`, and hover
/// displays both applicable constructor callables with bundle provenance.
#[test]
fn bundled_mock_flows_through_active_snapshot_into_hover() {
    let snapshot = Arc::new(
        basilisk_stubs::typeshed::bundle::bundled_snapshot()
            .expect("the release-attested bundled snapshot must activate"),
    );
    let source = "from unittest.mock import Mock\n\nmock = Mock()\n";
    let resolved = resolve_with_snapshot(Arc::clone(&snapshot), source);
    let mock = resolved
        .imported_symbols
        .get("Mock")
        .expect("the real named import must bind Mock");
    assert_eq!(
        mock.source_path.to_string_lossy(),
        snapshot
            .read_stub("unittest.mock")
            .map(|(path, _)| path)
            .expect("the bundled generation must contain unittest.mock"),
        "the exported declaration must retain the active generation URI"
    );
    assert!(
        mock.methods.iter().any(|method| method.name == "__new__"),
        "Mock must inherit NonCallableMock.__new__ over the real C3 hierarchy"
    );
    assert!(
        mock.methods.iter().any(|method| {
            method.name == "__init__" && method.signature.contains("side_effect")
        }),
        "Mock must inherit CallableMixin.__init__, not NonCallableMock/Base"
    );

    let offset = source.rfind("Mock").expect("usage must be present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]).expect("Mock usage must have hover");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("Mock hover must be Markdown");
    };
    assert!(
        markup.value.contains("def Mock.__new__("),
        "constructor conversion must retain the inherited __new__ alternative: {}",
        markup.value
    );
    assert!(
        markup.value.contains("def Mock.__init__(") && markup.value.contains("side_effect"),
        "constructor conversion must retain the inherited __init__ alternative: {}",
        markup.value
    );
    assert!(
        markup.value.contains("(typeshed)"),
        "hover provenance must identify the active bundled body: {}",
        markup.value
    );
}

/// [STUBRES-CUSTOM-TYPESHED] and #289: a user-managed step-3 generation is
/// canonical. Its conflicting Mock body replaces the bundle verbatim and the
/// hover provenance must not imply the built-in Typeshed snapshot.
#[test]
fn custom_snapshot_overrides_mock_constructor_body_and_provenance() {
    let snapshot = custom_mock_snapshot();
    let source = "from unittest.mock import Mock\n\nmock = Mock(b\"token\")\n";
    let resolved = resolve_with_snapshot(Arc::clone(&snapshot), source);
    let mock = resolved
        .imported_symbols
        .get("Mock")
        .expect("custom Mock must bind");
    assert!(mock
        .source_path
        .to_string_lossy()
        .starts_with("typeshed:custom-mock-override/"));

    let offset = source.rfind("Mock").expect("usage must be present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]).expect("custom Mock must have hover");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("custom Mock hover must be Markdown");
    };
    assert!(
        markup
            .value
            .contains("def Mock.__init__(custom_token: bytes, /) -> None"),
        "hover must render the conflicting custom body verbatim: {}",
        markup.value
    );
    assert!(!markup.value.contains("side_effect"), "{}", markup.value);
    assert!(
        markup.value.contains("(custom typeshed)"),
        "custom provenance must stay user-managed: {}",
        markup.value
    );
}
