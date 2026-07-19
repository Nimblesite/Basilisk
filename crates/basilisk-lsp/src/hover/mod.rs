//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//!
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
        push_symbol_sections(resolved, source, hit, &mut sections);
    }

    // 2b. Imported symbol with no local definition (e.g. a function/class from a
    // `.pyi` stub or a py.typed package): show its signature/type from
    // cross-module resolution so stub types surface on hover. When cross-module
    // resolution has not (yet) populated `imported_symbols`, fall back to the
    // import declaration itself, which is always available from the same-file
    // parse — so hovering a usage of an imported name is deterministic and never
    // races cross-file indexing.
    if hit.is_none() {
        if let Some(name) = identifier_at_offset(source, byte_offset) {
            let mut pushed = false;
            if let Some(ext_sym) = resolved.imported_symbols.get(&name) {
                let mut md = if let Some(sig) = &ext_sym.signature {
                    format!("```python\n{sig}\n```")
                } else if let Some(ty) = &ext_sym.type_annotation {
                    format!("```python\n{name}: {ty}\n```")
                } else {
                    String::new()
                };
                if let Some(label) = ext_sym
                    .provenance
                    .and_then(basilisk_stubs::TypeProvenance::hover_label)
                {
                    if !md.is_empty() {
                        md.push_str("\n\n");
                    }
                    let _ = write!(md, "*{label}*");
                }
                if !md.is_empty() {
                    sections.push(md);
                    pushed = true;
                }
                // An imported class also shows its constructor, resolved from the
                // real `.pyi` via the flattened MRO methods — its own or inherited
                // `__init__`/`__new__` (GitHub #289). Never a hand table.
                if ext_sym.kind == basilisk_resolver::scope::ExternalSymbolKind::Class {
                    for ctor in external_constructor_signatures(ext_sym) {
                        sections.push(format!("```python\n{ctor}\n```"));
                        pushed = true;
                    }
                }
            }
            if !pushed {
                if let Some(imp) = crate::util::find_import_by_bound_name(resolved, &name) {
                    let sig = format_type_signature(&SymbolHit::Import(imp), source);
                    sections.push(format!("```python\n{sig}\n```"));
                }
            }
        }
    }

    // 2c. Dot-access on a member of an external class (GitHub #287): e.g.
    // `Model.model_validate(...)` where `Model` subclasses an imported stub
    // class. Nothing local or top-level matched, so resolve the receiver to an
    // external class — directly or through local base chains — and show the
    // member's signature from the stub. Failing that, type the receiver as a
    // builtin (`" ".join(...)` — GitHub #288) and use the curated builtin
    // method signatures.
    if hit.is_none() && sections.is_empty() {
        if let Some(md) = external_member_hover(resolved, source, byte_offset)
            .or_else(|| builtin_member_hover(resolved, source, byte_offset))
        {
            sections.push(md);
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
            if let Some(link) = configure_severity_link(d.code.code) {
                let _ = write!(diag_md, "\n\n{link}");
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

/// Push the hover sections for a resolved symbol hit: its signature, the
/// constructor hint for classes (GitHub #289), its docstring, and the
/// provenance annotation for imported symbols.
fn push_symbol_sections(
    resolved: &ResolvedModule,
    source: &str,
    hit: &SymbolHit<'_>,
    sections: &mut Vec<String>,
) {
    let sig = format_type_signature(hit, source);
    sections.push(format!("```python\n{sig}\n```"));

    // A class hover includes its constructor so the user sees how to
    // instantiate it (GitHub #289): the class's own `__init__`, else the
    // nearest one in the local base chain.
    if let SymbolHit::Class(class) = hit {
        if let Some(init) = find_class_init(resolved, class) {
            let init_sig = format_type_signature(&SymbolHit::Function(init), source);
            sections.push(format!("```python\n{init_sig}\n```"));
        }
    }

    // Show docstring if available.
    let docstring = match hit {
        SymbolHit::Function(f) => f.docstring.as_deref(),
        SymbolHit::Class(c) => c.docstring.as_deref(),
        _ => None,
    };
    if let Some(ds) = docstring {
        sections.push(ds.to_owned());
    }

    // Show provenance annotation for imported symbols.
    let hit_name = match hit {
        SymbolHit::Function(f) => Some(f.name.as_str()),
        SymbolHit::Class(c) => Some(c.name.as_str()),
        SymbolHit::Variable(v) => Some(v.name.as_str()),
        _ => None,
    };
    if let Some(name) = hit_name {
        if let Some(ext_sym) = resolved.imported_symbols.get(name) {
            if let Some(label) = ext_sym
                .provenance
                .and_then(basilisk_stubs::TypeProvenance::hover_label)
            {
                sections.push(format!("*{label}*"));
            }
        }
    }
}

/// Render an imported class's constructor for hover ([STUBRES-PYI], GitHub #289).
///
/// Returns the callable union synthesized from the class's flattened `.pyi`
/// methods (its own or inherited over the C3 MRO). A metaclass `__call__` with
/// a non-instance return terminates conversion first. Otherwise every
/// `__new__` overload is bound and retained; when it returns the constructed
/// instance, every `__init__` overload is retained too. A non-instance
/// `__new__` terminates before `__init__`. The `object` fallback needs no extra
/// display.
fn external_constructor_signatures(ext: &basilisk_resolver::scope::ExternalSymbol) -> Vec<String> {
    if ext
        .metaclass_calls
        .iter()
        .any(|method| constructor_return_is_non_instance(&method.signature, &ext.name))
    {
        return ext
            .metaclass_calls
            .iter()
            .map(|method| qualify_constructor(&method.signature, &ext.name))
            .collect();
    }
    let news: Vec<_> = ext
        .methods
        .iter()
        .filter(|method| method.name == "__new__")
        .collect();
    let new_terminates = news
        .iter()
        .any(|method| constructor_return_is_non_instance(&method.signature, &ext.name));
    let methods = news.into_iter().chain(
        ext.methods
            .iter()
            .filter(|method| !new_terminates && method.name == "__init__"),
    );
    methods
        .map(|method| qualify_constructor(&method.signature, &ext.name))
        .collect()
}

/// Qualify a bound constructor method signature with the constructed class.
fn qualify_constructor(signature: &str, class_name: &str) -> String {
    signature.strip_prefix("def ").map_or_else(
        || signature.to_owned(),
        |rest| format!("def {class_name}.{rest}"),
    )
}

/// A union containing anything other than `Self` or the constructed class
/// makes metaclass `__call__` / `__new__` terminate constructor conversion.
fn constructor_return_is_non_instance(signature: &str, class_name: &str) -> bool {
    let Some(return_type) = signature.rsplit_once(" -> ").map(|(_, ty)| ty) else {
        return false;
    };
    constructor_union_members(return_type)
        .into_iter()
        .any(|member| !constructor_return_is_instance(member, class_name))
}

fn constructor_union_members(return_type: &str) -> Vec<&str> {
    let return_type = return_type.trim().trim_matches(['\'', '"']);
    let union_body = return_type
        .strip_prefix("Union[")
        .or_else(|| return_type.strip_prefix("typing.Union["))
        .and_then(|body| body.strip_suffix(']'));
    union_body.map_or_else(
        || return_type.split('|').map(str::trim).collect(),
        |body| body.split(',').map(str::trim).collect(),
    )
}

fn constructor_return_is_instance(return_type: &str, class_name: &str) -> bool {
    let return_type = return_type.trim().trim_matches(['\'', '"']);
    if return_type.rsplit('.').next() == Some("Self") {
        return true;
    }
    let head = return_type.split('[').next().unwrap_or(return_type).trim();
    head.rsplit('.').next() == Some(class_name)
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

// Implements [LSPUV-HOVER]
/// Format import resolution details as a Markdown snippet for hover display.
fn format_import_hover(import_info: &ImportInfo) -> String {
    let mut parts: Vec<String> = Vec::new();

    match import_info.resolution {
        ImportResolution::StubPyi => {
            parts.push("**Type stubs**: Available (.pyi)".to_owned());
        }
        ImportResolution::SourcePy => {
            parts.push("**Source**: .py *(no type stubs available)*".to_owned());
        }
        ImportResolution::Unresolved => {
            parts.push("**Status**: Unresolved *(no type stubs available)*".to_owned());
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

/// A "Configure Severity" command link for a non-PEP diagnostic `code`.
///
/// Opt-in Basilisk house rules can be graded or disabled per project, so
/// their hover deep-links into the configuration editor focused on the rule
/// ([CONFIGEDITOR-VSIX-EXPERIENCE]). PEP rules are graded by the typing spec
/// and never disabled ([CHKARCH-CONFIG-MODEL]) — they get no link.
///
/// The link is a `command:` URI whose argument list `[{"rule":"<code>"}]` is
/// percent-encoded per the VS Code command-URI contract. Rule codes are
/// static ASCII identifiers, so only the fixed JSON punctuation needs
/// encoding.
fn configure_severity_link(code: &str) -> Option<String> {
    if basilisk_checker::is_pep_rule(code) {
        return None;
    }
    let command = basilisk_common::configuration_editor::OPEN_EDITOR_COMMAND;
    Some(format!(
        "[Configure Severity](command:{command}?%5B%7B%22rule%22%3A%22{code}%22%7D%5D)"
    ))
}

pub(crate) mod members;

use members::{builtin_member_hover, external_member_hover, find_class_init};

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test-only code: expect/panic acceptable in unit tests"
)]
mod tests;

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test-only code: expect/panic acceptable in unit tests"
)]
mod tests_diagnostics;
