//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//! Acceptance for the shared-declaration consumer half of [TYPESHEDRT-ACCEPTANCE-HOVER].
//!
//! Hovering `receiver.member` must answer for **that member on that receiver**.
//! A plain `import os` publishes every member of `os` into the flat
//! `imported_symbols` map under its bare name, so typeshed's `error = OSError`
//! occupies the key `"error"`; hover used to answer `logger.error(...)` from
//! that key because it never looked at the receiver before the dot.
//!
//! These fixtures enter through the real salsa `cross_resolved_module` query
//! over the real bundled Typeshed snapshot — no hand-built `ExternalSymbol`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    reason = "end-to-end acceptance fixture"
)]

use std::sync::Arc;

use basilisk_checker::imports::ActiveTypeshed;
use basilisk_lsp::hover::hover_at;
use basilisk_stubs::types::{StubTarget, StubTargetPlatform};
use basilisk_test_utils::{cross_resolve, typeshed_search_paths};
use tower_lsp::lsp_types::HoverContents;

/// A module that logs through a `logging.Logger` while also importing `os` —
/// the exact collision that made hover answer with `os.error`.
const LOGGER_SOURCE: &str = concat!(
    "import logging\n",
    "import os\n",
    "\n",
    "logger = logging.getLogger(__name__)\n",
    "\n",
    "\n",
    "def mint() -> None:\n",
    "    logger.error(\"jwt_minter.no_secret\")\n",
);

fn resolve_against_bundled_typeshed(source: &str) -> Arc<basilisk_resolver::ResolvedModule> {
    let snapshot = Arc::new(
        basilisk_stubs::typeshed::bundle::bundled_snapshot()
            .expect("the release-attested bundled snapshot must activate"),
    );
    let search_paths = typeshed_search_paths(
        ActiveTypeshed::new(
            snapshot,
            Some(StubTarget {
                python_version: (3, 12),
                platform: StubTargetPlatform::Concrete("linux".to_owned()),
            }),
        ),
        Vec::new(),
    );
    cross_resolve(source, search_paths).expect("the fixture must parse and resolve")
}

fn hover_markdown(
    resolved: &basilisk_resolver::ResolvedModule,
    source: &str,
    offset: usize,
) -> Option<String> {
    match hover_at(resolved, source, offset, &[])?.contents {
        HoverContents::Markup(markup) => Some(markup.value),
        _ => panic!("hover contents must be Markdown"),
    }
}

/// The reported bug: hovering the `error` member of a `Logger` receiver
/// answered from typeshed's unrelated `os.error = OSError` binding, because
/// `identifier_at_offset` discards the `logger.` receiver and the bare-name
/// `imported_symbols` lookup pre-empted the dot-aware member lookup.
///
/// A member hover must never be answered by a bare module-level name.
#[test]
fn hover_on_member_never_answers_from_an_unrelated_bare_module_name() {
    let resolved = resolve_against_bundled_typeshed(LOGGER_SOURCE);
    assert!(
        resolved.imported_symbols.contains_key("error"),
        "precondition: a plain `import os` publishes typeshed's `error = OSError` \
         under the bare key, which is what hover used to answer with"
    );

    let offset = LOGGER_SOURCE
        .rfind("error")
        .expect("the member access must be present")
        + 1;
    let markdown = hover_markdown(&resolved, LOGGER_SOURCE, offset);

    assert!(
        !markdown.as_deref().is_some_and(|md| md.contains("OSError")),
        "hovering `logger.error` must not answer with `os.error`: {markdown:?}"
    );
}

/// [TYPESHEDRT-ACCEPTANCE-HOVER]: the receiver of a member access is typed
/// from the declaration that produced it — `logging.getLogger(...)` returns
/// `Logger` — so the hover shows `Logger.error`'s real typeshed signature,
/// labelled as a method and attributed to the active snapshot.
#[test]
fn hover_on_member_of_call_typed_receiver_shows_declaring_class_signature() {
    let resolved = resolve_against_bundled_typeshed(LOGGER_SOURCE);
    let offset = LOGGER_SOURCE
        .rfind("error")
        .expect("the member access must be present")
        + 1;
    let markdown = hover_markdown(&resolved, LOGGER_SOURCE, offset)
        .expect("a member of a typed receiver must have hover");

    assert!(
        markdown.contains("Logger.error"),
        "hover must qualify the member with the class that declares it: {markdown}"
    );
    assert!(
        markdown.contains("(method)"),
        "hover must say the symbol is a method: {markdown}"
    );
    assert!(
        markdown.contains("msg"),
        "hover must show the member's real parameters: {markdown}"
    );
    assert!(
        markdown.contains("(typeshed)"),
        "hover must attribute the declaration to the active snapshot: {markdown}"
    );
}

/// [LSPARCH-FEATURES-HOVER]: an imported symbol carries the same kind label a
/// local one does. `ExternalSymbol::kind` is known at hover time and was only
/// ever consulted to branch on classes, so imported functions rendered as a
/// bare signature with nothing saying what they are.
#[test]
fn hover_on_imported_stub_function_states_its_kind_and_origin() {
    let source = "from logging import getLogger\n\nlog = getLogger(__name__)\n";
    let resolved = resolve_against_bundled_typeshed(source);
    let offset = source
        .rfind("getLogger")
        .expect("the usage must be present")
        + 1;
    let markdown =
        hover_markdown(&resolved, source, offset).expect("an imported symbol must have hover");

    assert!(
        markdown.contains("(function)"),
        "an imported function must be labelled like a local one: {markdown}"
    );
    assert!(
        markdown.contains("def getLogger("),
        "the stub signature must still be shown: {markdown}"
    );
    assert!(
        markdown.contains("logging"),
        "hover must name the module the symbol came from: {markdown}"
    );
}
