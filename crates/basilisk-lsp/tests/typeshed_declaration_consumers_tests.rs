//! End-to-end acceptance for [STUBRES-PYI] #288 shared declarations.

use std::sync::Arc;

use basilisk_checker::imports::{ActiveTypeshed, ImportSearchPaths};
use basilisk_stubs::typeshed::snapshot::Snapshot;
use tower_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, Url};

fn resolve_with_snapshot(
    source: &str,
    snapshot: Arc<Snapshot>,
) -> Option<basilisk_resolver::ResolvedModule> {
    let parsed =
        basilisk_parser::parse_source(source.to_owned(), "/workspace/main.py".to_owned()).ok()?;
    let mut resolved = basilisk_resolver::resolve(&parsed).ok()?;
    let paths = ImportSearchPaths {
        roots: vec![std::path::PathBuf::from("/workspace")],
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(snapshot, None)),
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);
    Some(resolved)
}

fn hover_text(
    resolved: &basilisk_resolver::ResolvedModule,
    source: &str,
    offset: usize,
) -> Option<String> {
    let hover = basilisk_lsp::hover::hover_at(resolved, source, offset, &[])?;
    match hover.contents {
        HoverContents::Markup(markup) => Some(markup.value),
        _ => None,
    }
}

#[test]
fn bundled_str_join_one_declaration_drives_every_lsp_consumer() {
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot();
    assert!(
        snapshot.is_ok(),
        "release bundle must activate: {snapshot:?}"
    );
    let Ok(snapshot) = snapshot else { return };
    let identity = snapshot.identity.uri_component();
    let source = "items = ['a', 'b']\nvalue = ''.join(items)\n";
    let resolved = resolve_with_snapshot(source, Arc::new(snapshot));
    assert!(resolved.is_some(), "fixture must parse and resolve");
    let Some(resolved) = resolved else { return };
    let join_offset = source.rfind("join").map(|offset| offset + 1);
    assert!(join_offset.is_some(), "fixture must contain join");
    let Some(join_offset) = join_offset else {
        return;
    };

    let hover = hover_text(&resolved, source, join_offset);
    assert!(hover.is_some(), "built-in member hover must be markup");
    let Some(hover) = hover else { return };
    assert!(hover.contains("Iterable[LiteralString]"));
    assert!(hover.contains("-> LiteralString"));
    assert!(hover.contains("Iterable[str]"));
    assert!(hover.contains("-> str"));
    assert!(hover.contains('/'));
    assert!(hover.contains(&identity));

    let argument_offset = source.rfind("items)").map(|offset| offset + 2);
    assert!(
        argument_offset.is_some(),
        "fixture must contain call argument"
    );
    let Some(argument_offset) = argument_offset else {
        return;
    };
    let signature = basilisk_lsp::signature::signature_help_at(&resolved, source, argument_offset);
    assert!(signature.is_some(), "built-in signature help must exist");
    let Some(signature) = signature else { return };
    assert_eq!(signature.signatures.len(), 2);
    let first_signature = signature.signatures.first();
    assert!(first_signature.is_some(), "first overload must exist");
    let Some(first_signature) = first_signature else {
        return;
    };
    assert!(first_signature.label.contains("LiteralString"));
    assert!(first_signature.label.contains('/'));

    let completion_source = "''.jo";
    let completion =
        basilisk_lsp::completion::complete(&resolved, completion_source, completion_source.len())
            .into_iter()
            .find(|item| item.label == "join");
    assert!(completion.is_some(), "indexed class must complete join");
    let Some(completion) = completion else { return };
    assert!(completion
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("LiteralString")));
    assert_eq!(
        completion
            .data
            .as_ref()
            .and_then(|data| data.get("sourceIdentity"))
            .and_then(serde_json::Value::as_str),
        Some(identity.as_str())
    );

    let uri = Url::parse("file:///workspace/main.py");
    assert!(uri.is_ok(), "fixture file URI must parse: {uri:?}");
    let Ok(uri) = uri else { return };
    let definition =
        basilisk_lsp::definition::goto_definition(&resolved, source, join_offset, &uri);
    assert!(definition.is_some(), "built-in definition must exist");
    assert!(
        matches!(&definition, Some(GotoDefinitionResponse::Scalar(_))),
        "expected one declaration location"
    );
    let Some(GotoDefinitionResponse::Scalar(location)) = definition else {
        return;
    };
    assert_eq!(location.uri.scheme(), "typeshed");
    assert!(location.uri.as_str().contains(&identity));
    assert!(location.range.start.line > 0);
}

#[test]
fn custom_builtins_mutation_replaces_every_consumer_without_bundle_mixing() {
    let root = tempfile::tempdir();
    assert!(root.is_ok(), "temporary custom typeshed: {root:?}");
    let Ok(root) = root else { return };
    let setup = std::fs::create_dir(root.path().join("stdlib")).and_then(|()| {
        std::fs::write(
            root.path().join("stdlib/builtins.pyi"),
            "class str:\n    def join(self, token: int, /) -> bytes: ...\n",
        )
    });
    assert!(setup.is_ok(), "custom Typeshed setup failed: {setup:?}");
    if setup.is_err() {
        return;
    }
    let request = basilisk_stubs::typeshed::source::TypeshedRequest {
        selection: basilisk_stubs::typeshed::source::SourceSelection::Custom {
            path: root.path().to_string_lossy().into_owned(),
        },
        verify_content: true,
        use_cache: false,
        url_template: None,
    };
    let manager = basilisk_stubs::typeshed::runtime::production_manager(request, None);
    assert!(manager.is_ok(), "custom manager must initialize");
    let Ok(manager) = manager else { return };
    let snapshot = manager.snapshot();
    assert!(
        snapshot.is_ok(),
        "custom generation must activate: {snapshot:?}"
    );
    let Ok(snapshot) = snapshot else { return };
    let identity = snapshot.identity.uri_component();
    let source = "value = ''.join(1)\n";
    let resolved = resolve_with_snapshot(source, snapshot);
    assert!(resolved.is_some(), "fixture must parse and resolve");
    let Some(resolved) = resolved else { return };
    let join_offset = source.find("join").map(|offset| offset + 1);
    assert!(join_offset.is_some(), "fixture must contain join");
    let Some(join_offset) = join_offset else {
        return;
    };

    let hover = hover_text(&resolved, source, join_offset);
    assert!(hover.is_some(), "custom member hover must be markup");
    let Some(hover) = hover else { return };
    assert!(hover.contains("token: int"));
    assert!(hover.contains("-> bytes"));
    assert!(hover.contains("custom typeshed"));
    assert!(hover.contains(&identity));
    assert!(!hover.contains("Iterable[str]"));

    let argument_offset = source.find("1)").map(|offset| offset + 1);
    assert!(argument_offset.is_some(), "fixture must contain argument");
    let Some(argument_offset) = argument_offset else {
        return;
    };
    let signature = basilisk_lsp::signature::signature_help_at(&resolved, source, argument_offset);
    assert!(signature.is_some(), "custom signature help must exist");
    let Some(signature) = signature else { return };
    assert_eq!(signature.signatures.len(), 1);
    let first_signature = signature.signatures.first();
    assert!(first_signature.is_some(), "custom signature must exist");
    let Some(first_signature) = first_signature else {
        return;
    };
    assert_eq!(first_signature.label, "join(token: int, /) -> bytes");

    let completion_source = "''.jo";
    let completion =
        basilisk_lsp::completion::complete(&resolved, completion_source, completion_source.len())
            .into_iter()
            .find(|item| item.label == "join");
    assert!(completion.is_some(), "custom join completion must exist");
    let Some(completion) = completion else { return };
    assert_eq!(
        completion.detail.as_deref(),
        Some("def join(token: int, /) -> bytes")
    );

    let uri = Url::parse("file:///workspace/main.py");
    assert!(uri.is_ok(), "fixture file URI must parse: {uri:?}");
    let Ok(uri) = uri else { return };
    let definition =
        basilisk_lsp::definition::goto_definition(&resolved, source, join_offset, &uri);
    assert!(definition.is_some(), "custom definition must exist");
    assert!(
        matches!(&definition, Some(GotoDefinitionResponse::Scalar(_))),
        "expected one custom declaration location"
    );
    let Some(GotoDefinitionResponse::Scalar(location)) = definition else {
        return;
    };
    assert!(location.uri.as_str().contains(&identity));
    assert!(!location.uri.as_str().contains("bundled-"));
}
