//! Hover handler: type-aware hover with diagnostic and import info.
//!
//! Shows type signatures for symbols at definition sites, reference sites
//! (call sites, variable uses), and dot-access sites (`self.attr`).
//! When hovering over an import statement, shows resolution status and path.

use std::fmt::Write as _;

use basilisk_resolver::{ImportInfo, ImportResolution, PackageDepKind, ResolvedModule};
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::util::{
    find_definition_by_name, find_symbol_at_offset, format_type_signature, identifier_at_offset,
    SymbolHit,
};

/// Compute hover information at a byte offset.
///
/// Searches definition sites first, then tries name-based lookup for
/// reference sites (call sites, variable uses). Also shows import resolution
/// details when the cursor is on an import statement, and any diagnostics
/// covering the cursor position.
#[must_use]
pub fn hover_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    diagnostics: &[basilisk_checker::Diagnostic],
) -> Option<Hover> {
    let mut sections: Vec<String> = Vec::new();

    // 1. Definition site: cursor directly on a symbol's name_span.
    let hit = find_symbol_at_offset(resolved, byte_offset);

    // 2. Reference site: cursor on an identifier, look up by name.
    let hit = hit.or_else(|| {
        let name = identifier_at_offset(source, byte_offset)?;
        find_definition_by_name(resolved, &name)
    });

    if let Some(ref hit) = hit {
        let sig = format_type_signature(hit, source);
        sections.push(format!("```python\n{sig}\n```"));

        // Show docstring if available.
        let docstring = match hit {
            SymbolHit::Function(f) => f.docstring.as_deref(),
            SymbolHit::Class(c) => c.docstring.as_deref(),
            _ => None,
        };
        if let Some(ds) = docstring {
            sections.push(ds.to_owned());
        }
    }

    // 3. Import resolution details when the cursor is on an import statement.
    if let Some(import_info) = find_import_at_offset(resolved, byte_offset) {
        let import_md = format_import_hover(import_info);
        if !import_md.is_empty() {
            sections.push(import_md);
        }
    }

    // Diagnostic info at this position.
    for d in diagnostics {
        if d.span.start_usize() <= byte_offset && byte_offset < d.span.end_usize() {
            let mut diag_md = format!("**{}** — {}", d.code.code, d.message);
            if let Some(ref help) = d.help {
                let _ = write!(diag_md, "\n\n_{help}_");
            }
            sections.push(diag_md);
        }
    }

    if sections.is_empty() {
        return None;
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: sections.join("\n\n---\n\n"),
        }),
        range: None,
    })
}

/// Find an import at the given byte offset.
///
/// Returns the first `ImportInfo` whose source span contains `byte_offset`,
/// or `None` if the cursor is not on an import statement.
fn find_import_at_offset(resolved: &ResolvedModule, byte_offset: usize) -> Option<&ImportInfo> {
    resolved.imports.iter().find(|import| {
        let start = import.span.start_usize();
        let end = import.span.end_usize();
        byte_offset >= start && byte_offset < end
    })
}

/// Format import resolution details as a Markdown snippet for hover display.
fn format_import_hover(import_info: &ImportInfo) -> String {
    let mut parts: Vec<String> = Vec::new();

    match import_info.resolution {
        ImportResolution::StubPyi => {
            parts.push("**Type stubs**: Available (.pyi)".to_owned());
        }
        ImportResolution::SourcePy => {
            parts.push("**Source**: .py (no type stubs)".to_owned());
        }
        ImportResolution::Unresolved => {
            parts.push("**Status**: Unresolved".to_owned());
        }
    }

    // Show package name and version from the uv registry.
    if let Some(ref version) = import_info.package_version {
        let display_name = import_info
            .package_name
            .as_deref()
            .unwrap_or(&import_info.module);
        parts.push(format!("**Package**: {display_name} v{version}"));
    }

    // Show dependency classification from uv registry.
    if let Some(ref dep_kind) = import_info.package_dep_kind {
        let label = match dep_kind {
            PackageDepKind::Direct => "direct dependency",
            PackageDepKind::Dev => "dev dependency",
            PackageDepKind::Transitive => "transitive dependency",
        };
        parts.push(format!("**Dependency**: {label}"));
    }

    if let Some(path) = &import_info.resolved_path {
        // Detect workspace member imports by checking for absence of `site-packages`.
        let path_str = path.display().to_string();
        if !path_str.contains("site-packages") && import_info.package_dep_kind.is_none() {
            parts.push(format!("**Workspace member**: `{path_str}`"));
        } else {
            parts.push(format!("**Path**: `{path_str}`"));
        }
    }

    parts.join("\n\n")
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test-only code: expect/panic acceptable in unit tests"
)]
mod tests {
    use super::*;

    /// Parse and resolve source code, then patch the first import's resolution
    /// and path fields for testing hover display.
    fn resolve_with_patched_import(
        source: &str,
        resolution: ImportResolution,
        resolved_path: Option<std::path::PathBuf>,
    ) -> ResolvedModule {
        let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())
            .expect("test source should parse");
        let mut resolved = basilisk_resolver::resolve(&parsed).expect("resolution should not fail");
        if let Some(import) = resolved.imports.first_mut() {
            import.resolution = resolution;
            import.resolved_path = resolved_path;
        }
        resolved
    }

    #[test]
    fn test_hover_on_resolved_import_shows_stub_info() {
        let source = "import os\n";
        let resolved = resolve_with_patched_import(
            source,
            ImportResolution::StubPyi,
            Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
        );

        let hover = hover_at(&resolved, source, 7, &[]);
        let hover = hover.expect("hover should be Some for resolved import");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected Markup hover contents");
        };

        assert!(
            markup.value.contains("Type stubs"),
            "should mention type stubs: {}",
            markup.value
        );
        assert!(
            markup.value.contains("os.pyi"),
            "should show resolved path: {}",
            markup.value
        );
    }

    #[test]
    fn test_hover_on_unresolved_import_shows_status() {
        let source = "import nonexistent\n";
        let resolved = resolve_with_patched_import(source, ImportResolution::Unresolved, None);

        let hover = hover_at(&resolved, source, 10, &[]);
        let hover = hover.expect("hover should be Some for unresolved import");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected Markup hover contents");
        };

        assert!(
            markup.value.contains("Unresolved"),
            "should show Unresolved status: {}",
            markup.value
        );
    }

    #[test]
    fn test_hover_on_source_py_import_shows_no_stubs() {
        let source = "import mymodule\n";
        let resolved = resolve_with_patched_import(
            source,
            ImportResolution::SourcePy,
            Some(std::path::PathBuf::from("/project/mymodule.py")),
        );

        let hover = hover_at(&resolved, source, 10, &[]);
        let hover = hover.expect("hover should be Some for source import");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected Markup hover contents");
        };

        assert!(
            markup.value.contains("no type stubs"),
            "should indicate no type stubs: {}",
            markup.value
        );
        assert!(
            markup.value.contains("mymodule.py"),
            "should show resolved path: {}",
            markup.value
        );
    }

    #[test]
    fn test_hover_outside_import_span_returns_none_for_import() {
        let source = "x: int = 1\nimport os\n";
        let resolved = resolve_with_patched_import(
            source,
            ImportResolution::StubPyi,
            Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
        );

        // Offset 5 is on "x: int = 1", not on the import.
        let import_hit = find_import_at_offset(&resolved, 5);
        assert!(
            import_hit.is_none(),
            "should not find import outside its span"
        );
    }

    #[test]
    fn test_hover_shows_package_version_and_name() {
        let source = "import requests\n";
        let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())
            .expect("test source should parse");
        let mut resolved = basilisk_resolver::resolve(&parsed).expect("resolution should not fail");
        if let Some(import) = resolved.imports.first_mut() {
            import.resolution = ImportResolution::SourcePy;
            import.resolved_path = Some(std::path::PathBuf::from(
                "/venv/lib/python3.12/site-packages/requests/__init__.py",
            ));
            import.package_name = Some("requests".to_owned());
            import.package_version = Some("2.31.0".to_owned());
            import.package_dep_kind = Some(PackageDepKind::Direct);
        }

        let hover = hover_at(&resolved, source, 10, &[]);
        let hover = hover.expect("hover should be Some");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected Markup hover contents");
        };

        assert!(
            markup.value.contains("requests v2.31.0"),
            "should show package name and version: {}",
            markup.value
        );
        assert!(
            markup.value.contains("direct dependency"),
            "should show dependency kind: {}",
            markup.value
        );
    }

    #[test]
    fn test_hover_no_package_info_for_stdlib() {
        let source = "import os\n";
        let resolved = resolve_with_patched_import(
            source,
            ImportResolution::StubPyi,
            Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
        );

        let hover = hover_at(&resolved, source, 7, &[]);
        let hover = hover.expect("hover should be Some");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected Markup hover contents");
        };

        // stdlib imports should not show package version or dependency kind.
        assert!(
            !markup.value.contains("**Package**"),
            "stdlib should not show package info: {}",
            markup.value
        );
        assert!(
            !markup.value.contains("**Dependency**"),
            "stdlib should not show dependency kind: {}",
            markup.value
        );
    }
}
