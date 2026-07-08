//! Hover coverage for [STUBRES-CUSTOM-TYPESHED].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED

#![allow(missing_docs)]

use basilisk_lsp::hover::hover_at;
use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind};
use basilisk_resolver::Span;
use tower_lsp::lsp_types::HoverContents;

fn parse_and_resolve(source: &str) -> Result<basilisk_resolver::ResolvedModule, String> {
    let parsed =
        basilisk_parser::parse_source(source.to_owned(), "hover_custom_typeshed.py".to_owned())
            .map_err(|error| error.to_string())?;
    basilisk_resolver::resolve(&parsed).map_err(|error| error.to_string())
}

/// Custom-typeshed hovers must say `(custom typeshed)`, never the bundled
/// `(typeshed)` label, so a `MicroPython` signature is not misreported as the
/// bundled `CPython` one ([STUBRES-CUSTOM-TYPESHED]).
#[test]
fn hover_on_custom_typeshed_symbol_shows_custom_annotation() -> Result<(), String> {
    let source = "from os import uname\n\nname = uname()\n";
    let mut resolved = parse_and_resolve(source)?;

    let _ = resolved.imported_symbols.insert(
        "uname".to_owned(),
        ExternalSymbol {
            name: "uname".to_owned(),
            kind: ExternalSymbolKind::Function,
            type_annotation: Some("str".to_owned()),
            source_path: std::path::PathBuf::from("/proj/ts/stdlib/os.pyi"),
            source_span: Span::new(0, 0),
            signature: Some("def uname() -> str".to_owned()),
            provenance: Some(basilisk_stubs::TypeProvenance::StubCustomTypeshed),
            methods: Vec::new(),
        },
    );

    let offset = source
        .rfind("uname")
        .ok_or_else(|| "usage should be present".to_owned())?
        + 1;
    let hover = hover_at(&resolved, source, offset, &[])
        .ok_or_else(|| "hover should be Some for a custom-typeshed symbol".to_owned())?;
    let HoverContents::Markup(markup) = hover.contents else {
        return Err("expected Markup hover contents".to_owned());
    };

    assert!(
        markup.value.contains("(custom typeshed)"),
        "hover must show the '(custom typeshed)' provenance annotation: {}",
        markup.value
    );
    assert!(
        !markup.value.contains("(typeshed)"),
        "custom-typeshed hover must not render the bundled '(typeshed)' label: {}",
        markup.value
    );
    assert!(
        markup.value.contains("def uname() -> str"),
        "hover should still show the stub signature: {}",
        markup.value
    );
    Ok(())
}
