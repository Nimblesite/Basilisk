//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//!
//! Dot-access member hover lookups: methods inherited from external
//! (stub/py.typed) base classes (GitHub #287), builtin-typed receivers like
//! `" ".join(...)` (GitHub #288), and the class-constructor hint (GitHub #289).

use std::fmt::Write as _;

use basilisk_resolver::ResolvedModule;

use crate::util::{find_definition_by_name, identifier_at_offset, SymbolHit};

/// Hover markdown for `Receiver.member` where `member` lives on an external
/// class — the receiver itself or a (transitive) base of a local class.
///
/// Implements the dot-access member lookup for GitHub #287: methods inherited
/// from stub/py.typed base classes have no local definition, so the standard
/// symbol lookups find nothing.
pub(super) fn external_member_hover(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<String> {
    let member = identifier_at_offset(source, byte_offset)?;
    let receiver = crate::completion::prefix::dot_receiver(source, byte_offset)?;
    let (class, method) = find_external_member(resolved, &receiver, &member)?;

    // Stub signatures render as `def name(...)`; qualify with the class name
    // to match the local-method hover style `(method) def Class.name(...)`.
    let qualified = method.signature.strip_prefix("def ").map_or_else(
        || method.signature.clone(),
        |rest| format!("def {}.{rest}", class.name),
    );
    let mut md = format!("```python\n(method) {qualified}\n```");
    if let Some(label) = class
        .provenance
        .and_then(basilisk_stubs::TypeProvenance::hover_label)
    {
        let _ = write!(md, "\n\n*{label}*");
    }
    Some(md)
}

/// Find `member` on an external class reachable from `receiver`.
///
/// `receiver` may name an imported class directly, or a local class whose
/// (transitive) bases include one.
fn find_external_member<'a>(
    resolved: &'a ResolvedModule,
    receiver: &str,
    member: &str,
) -> Option<(
    &'a basilisk_resolver::scope::ExternalSymbol,
    &'a basilisk_resolver::scope::ExternalMethod,
)> {
    use basilisk_resolver::scope::ExternalSymbolKind;

    walk_class_hierarchy(resolved, receiver, |class_name| {
        let ext = resolved.imported_symbols.get(class_name)?;
        if ext.kind != ExternalSymbolKind::Class {
            return None;
        }
        let method = ext.methods.iter().find(|m| m.name == member)?;
        Some((ext, method))
    })
}

/// Find the `__init__` a class would run: its own, else the nearest one in
/// its local base chain (GitHub #289).
pub(super) fn find_class_init<'a>(
    resolved: &'a ResolvedModule,
    class: &basilisk_resolver::ClassInfo,
) -> Option<&'a basilisk_resolver::FunctionInfo> {
    walk_class_hierarchy(resolved, &class.name, |class_name| {
        resolved
            .functions
            .iter()
            .find(|func| func.name == "__init__" && func.class_name.as_deref() == Some(class_name))
    })
}

/// Visit `start` and its transitive local base classes breadth-first
/// (left-to-right, approximating the MRO), returning the visitor's first
/// `Some`. The walk is cycle-guarded — base names are recorded unqualified,
/// so `class Client(httpx.Client)` makes a class look like its own base
/// (GitHub #278). Subscripts are stripped so `Base[int]` resolves as `Base`.
fn walk_class_hierarchy<'r, T>(
    resolved: &'r ResolvedModule,
    start: &'r str,
    mut visit: impl FnMut(&str) -> Option<T>,
) -> Option<T> {
    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::from([start]);
    while let Some(class_name) = queue.pop_front() {
        if !visited.insert(class_name) {
            continue;
        }
        if let Some(found) = visit(class_name) {
            return Some(found);
        }
        if let Some(local) = resolved.classes.iter().find(|c| c.name == class_name) {
            queue.extend(
                local
                    .bases
                    .iter()
                    .map(|base| base.split('[').next().unwrap_or(base)),
            );
        }
    }
    None
}

/// Hover markdown for `recv.member` where `recv` is a builtin-typed receiver.
/// Every overload comes from the structured declaration indexed from the
/// active snapshot's real `builtins.pyi` body (GitHub #288).
pub(super) fn builtin_member_hover(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<String> {
    let member = identifier_at_offset(source, byte_offset)?;
    let (class, declarations) =
        builtin_member_declarations(resolved, source, byte_offset, &member)?;
    let signatures = declarations
        .iter()
        .map(|declaration| basilisk_stubs::render_stub_signature(declaration))
        .map(|signature| {
            format!(
                "(method) def {}.{rest}",
                class.declaration.name,
                rest = signature.strip_prefix("def ").unwrap_or(&signature)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut markdown = format!("```python\n{signatures}\n```");
    if let Some(label) = class.provenance.hover_label() {
        let _ = write!(markdown, "\n\n*{label}* — `{}`", class.source_identity);
    }
    Some(markdown)
}

/// Active structured declarations for the built-in member at an editor offset.
pub(crate) fn builtin_member_declarations<'a>(
    resolved: &'a ResolvedModule,
    source: &str,
    byte_offset: usize,
    member: &str,
) -> Option<(
    &'a basilisk_resolver::scope::IndexedStubClass,
    Vec<&'a basilisk_stubs::StubFunction>,
)> {
    let (type_name, literal_receiver) = dot_receiver_builtin_type(resolved, source, byte_offset)?;
    let class = resolved.builtin_classes.get(&type_name)?;
    let declarations = class
        .declaration
        .methods
        .iter()
        .filter(|method| method.name == member)
        .filter(|method| {
            literal_receiver
                || method
                    .receiver
                    .as_ref()
                    .and_then(|receiver| receiver.annotation.as_deref())
                    .is_none_or(|annotation| !annotation.contains("LiteralString"))
        })
        .collect::<Vec<_>>();
    (!declarations.is_empty()).then_some((class, declarations))
}

/// The builtin type name of the receiver before the `.` at `byte_offset`,
/// or `None` when it cannot be determined.
pub(crate) fn dot_receiver_builtin_type(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<(String, bool)> {
    let before = source.get(..byte_offset.min(source.len()))?;
    let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    let receiver_text = trimmed.strip_suffix('.')?;
    match receiver_text.chars().last()? {
        quote @ ('"' | '\'') if str_literal_receiver(receiver_text, quote) => {
            Some(("str".to_owned(), true))
        }
        c if c.is_alphanumeric() || c == '_' => {
            let receiver = crate::completion::prefix::dot_receiver(source, byte_offset)?;
            variable_builtin_type(resolved, source, &receiver)
        }
        _ => None,
    }
}

/// Best-effort check that the text ending in `quote` closes a *str* literal —
/// walks back to the matching opening quote and rejects a `b`/`B` prefix, so
/// a bytes literal never hovers with `str` signatures.
fn str_literal_receiver(receiver_text: &str, quote: char) -> bool {
    let body = &receiver_text[..receiver_text.len() - quote.len_utf8()];
    let Some(open) = body.rfind(quote) else {
        return false;
    };
    !body[..open]
        .trim_end_matches(['r', 'R'])
        .ends_with(['b', 'B'])
}

/// The builtin type of a named variable or parameter: its annotation when it
/// names a builtin type, else its inferred right-hand-side type.
fn variable_builtin_type(
    resolved: &ResolvedModule,
    source: &str,
    receiver: &str,
) -> Option<(String, bool)> {
    let (annotation_span, rhs_kind) = match find_definition_by_name(resolved, receiver)? {
        SymbolHit::Variable(var) => (var.annotation_span, Some(&var.rhs_kind)),
        SymbolHit::Parameter { param, .. } => (param.annotation_span, None),
        _ => return None,
    };
    if let Some(annotation) = crate::util::annotation_text(annotation_span, source) {
        let literal = annotation == "LiteralString" || annotation == "typing.LiteralString";
        let type_name = if literal {
            "str".to_owned()
        } else {
            annotation
        };
        return Some((type_name, literal));
    }
    let inferred = crate::util::rhs_type_display(rhs_kind?);
    if inferred.is_empty() {
        None
    } else {
        let literal =
            inferred == "str" && matches!(rhs_kind, Some(basilisk_resolver::RhsKind::StrLiteral));
        Some((inferred, literal))
    }
}
