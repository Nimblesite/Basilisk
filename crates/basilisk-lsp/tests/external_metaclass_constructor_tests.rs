//! Active-snapshot acceptance for external `.pyi` metaclass constructor conversion (#289).

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

fn metaclass_snapshot() -> Arc<Snapshot> {
    let identity = SourceIdentity::Custom {
        digest: "external-metaclass".to_owned(),
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
            data: concat!(
                "class SpecialMeta(type):\n",
                "    def __call__(cls, token: int) -> Never: ...\n",
                "\n",
                "class Special(metaclass=SpecialMeta):\n",
                "    def __new__(cls, ignored_new: bytes) -> Self: ...\n",
                "    def __init__(self, ignored_init: str) -> None: ...\n",
                "\n",
                "class OrdinaryMeta(type):\n",
                "    def __call__(cls, *args: object, **kwargs: object) -> Self: ...\n",
                "\n",
                "class Ordinary(metaclass=OrdinaryMeta):\n",
                "    def __new__(cls, created: bytes) -> Self: ...\n",
                "    def __init__(self, required: int) -> None: ...\n",
            )
            .as_bytes()
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
        .expect("metaclass fixture snapshot must activate"),
    )
}

fn resolve(source: &str) -> Arc<basilisk_resolver::ResolvedModule> {
    let search_paths = ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(
            metaclass_snapshot(),
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
        panic!("fixture source must parse and resolve");
    };
    Arc::clone(resolved)
}

fn hover_markdown(
    resolved: &basilisk_resolver::ResolvedModule,
    source: &str,
    class_name: &str,
) -> String {
    let offset = source.rfind(class_name).expect("class use must be present") + 1;
    let hover = hover_at(resolved, source, offset, &[]).expect("class use must have hover");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("class hover must be Markdown");
    };
    markup.value
}

#[test]
fn external_special_metaclass_call_terminates_but_instance_call_does_not() {
    let source = "from unittest.mock import Special, Ordinary\n\nspecial = Special(token=1)\nordinary = Ordinary(b'x')\n";
    let resolved = resolve(source);

    let special = resolved.imported_symbols.get("Special").unwrap_or_else(|| {
        panic!(
            "Special import must bind; imports={:#?}; symbols={:#?}",
            resolved.imports,
            resolved.imported_symbols.keys().collect::<Vec<_>>()
        )
    });
    assert_eq!(special.metaclass.as_deref(), Some("SpecialMeta"));
    let special_hover = hover_markdown(&resolved, source, "Special");
    assert!(special_hover.contains("def Special.__call__(token: int) -> Never"));
    assert!(!special_hover.contains("ignored_new"), "{special_hover}");
    assert!(!special_hover.contains("ignored_init"), "{special_hover}");

    let ordinary = resolved
        .imported_symbols
        .get("Ordinary")
        .expect("Ordinary import must bind");
    assert_eq!(ordinary.metaclass.as_deref(), Some("OrdinaryMeta"));
    let ordinary_hover = hover_markdown(&resolved, source, "Ordinary");
    assert!(ordinary_hover.contains("def Ordinary.__new__(created: bytes) -> Self"));
    assert!(ordinary_hover.contains("def Ordinary.__init__(required: int) -> None"));
    assert!(
        !ordinary_hover.contains("Ordinary.__call__"),
        "{ordinary_hover}"
    );
}
