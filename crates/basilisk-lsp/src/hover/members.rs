//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//! Implements the shared-declaration consumer half of [TYPESHEDRT-ACCEPTANCE-HOVER].
//!
//! Member-access hover: `receiver.member`.
//!
//! The receiver decides the answer. It may name an imported module
//! (`os.getcwd`), a class (`Model.model_validate` — GitHub #287), or a value
//! whose type comes from its annotation, its literal, or the call that
//! produced it (`logger = logging.getLogger(...)` → `Logger`). A receiver
//! nothing can type yields no hover: answering from the flat `imported_symbols`
//! map by bare name would name whichever module happened to export the same
//! word last.

use basilisk_resolver::ResolvedModule;

use super::access::{self, Access};
use super::receiver_scope;
use super::render::{SymbolCard, SymbolKind};
use crate::util::identifier_at_offset;

/// Hover markdown for the member access at `byte_offset`.
///
/// The first resolution that names a real declaration wins: the module the
/// receiver binds, a class in the local hierarchy, an external (stub or
/// `py.typed`) class, then a built-in type.
pub(super) fn member_hover(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    access: &Access,
) -> Option<String> {
    let member = identifier_at_offset(source, byte_offset)?;
    let receiver = access.receiver();
    let class_name =
        receiver.and_then(|receiver| receiver_class_name(resolved, source, byte_offset, receiver));

    receiver
        .and_then(|receiver| module_member_hover(resolved, receiver, &member))
        .or_else(|| local_member_hover(resolved, class_name.as_deref(), &member))
        .or_else(|| external_member_hover(resolved, receiver, class_name.as_deref(), &member))
        .or_else(|| builtin_member_hover(resolved, source, byte_offset, &member))
}

/// The class whose members a receiver exposes.
///
/// `self`/`cls` expose the enclosing class; a receiver naming a local class
/// exposes that class's own members (`Model.model_validate`); anything else is
/// a value, typed from its own declaration.
fn receiver_class_name(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    receiver: &str,
) -> Option<String> {
    if matches!(receiver, "self" | "cls") {
        return access::enclosing_class(resolved, byte_offset).map(|class| class.name.clone());
    }
    if resolved.classes.iter().any(|class| class.name == receiver) {
        return Some(receiver.to_owned());
    }
    access::receiver_type_name(resolved, source, receiver).map(|(name, _)| name)
}

/// Hover for `module.member`, e.g. `os.getcwd`.
fn module_member_hover(resolved: &ResolvedModule, receiver: &str, member: &str) -> Option<String> {
    let symbol = access::module_member(resolved, receiver, member)?;
    super::external_symbol_card(symbol, Some(receiver.to_owned())).render()
}

/// Hover for a member declared by a class in the *local* hierarchy — its own
/// method or attribute, or one inherited from a local base.
fn local_member_hover(
    resolved: &ResolvedModule,
    class_name: Option<&str>,
    member: &str,
) -> Option<String> {
    let start = class_name?;
    let hit = walk_class_hierarchy(resolved, start, |name| {
        resolved
            .functions
            .iter()
            .find(|func| func.name == member && func.class_name.as_deref() == Some(name))
            .map(crate::util::SymbolHit::Function)
            .or_else(|| {
                resolved
                    .classes
                    .iter()
                    .find(|class| class.name == name)
                    .and_then(|class| {
                        class
                            .attributes
                            .iter()
                            .find(|attr| attr.name == member)
                            .map(|attr| crate::util::SymbolHit::Attribute { class, attr })
                    })
            })
    })?;
    let signature = crate::util::format_type_signature(&hit, resolved);
    let docstring = match hit {
        crate::util::SymbolHit::Function(func) => func.docstring.clone(),
        _ => None,
    };
    // `format_type_signature` already labels local symbols with their kind, so
    // the card contributes no second prefix.
    SymbolCard::new(None, signature)
        .documented(docstring)
        .render()
}

/// Hover markdown for a member declared on an external (stub or `py.typed`)
/// class — the receiver's own class, or a (transitive) base of it.
///
/// Implements the dot-access member lookup for GitHub #287: methods inherited
/// from stub base classes have no local definition, so the standard symbol
/// lookups find nothing. Every overload of the member is shown, matching the
/// built-in path.
fn external_member_hover(
    resolved: &ResolvedModule,
    receiver: Option<&str>,
    class_name: Option<&str>,
    member: &str,
) -> Option<String> {
    let (class, methods) = receiver
        .and_then(|receiver| find_external_member(resolved, receiver, member))
        .or_else(|| class_name.and_then(|name| find_external_member(resolved, name, member)))?;

    // Stub signatures render as `def name(...)`; qualify with the class name
    // so the hover names the type that declares the member.
    let signatures = methods
        .iter()
        .map(|method| {
            method.signature.strip_prefix("def ").map_or_else(
                || method.signature.clone(),
                |rest| format!("def {}.{rest}", class.name),
            )
        })
        .collect();
    SymbolCard {
        kind: Some(SymbolKind::Method),
        signatures,
        ..SymbolCard::default()
    }
    .documented(methods.iter().find_map(|method| method.docstring.clone()))
    .declared_in(
        declaring_module(resolved, class.source_path.as_path()),
        class.provenance.as_ref(),
        Some(class.source_path.as_path()),
    )
    .render()
}

/// Find `member` on an external class reachable from `start`.
///
/// `start` may name an imported class directly, or a local class whose
/// (transitive) bases include one. Returns every overload of the member.
fn find_external_member<'a>(
    resolved: &'a ResolvedModule,
    start: &'a str,
    member: &str,
) -> Option<(
    &'a basilisk_resolver::scope::ExternalSymbol,
    Vec<&'a basilisk_resolver::scope::ExternalMethod>,
)> {
    use basilisk_resolver::scope::ExternalSymbolKind;

    walk_class_hierarchy(resolved, start, |class_name| {
        let ext = resolved.imported_symbols.get(class_name)?;
        if ext.kind != ExternalSymbolKind::Class {
            return None;
        }
        let methods: Vec<_> = ext.methods.iter().filter(|m| m.name == member).collect();
        (!methods.is_empty()).then_some((ext, methods))
    })
}

/// The module an external declaration was read from, for the origin line.
///
/// Matched by resolved file, not by name: a class reached through a receiver's
/// type (`Logger` from `logging.getLogger(...)`) is never itself a bound name,
/// so a name lookup would find nothing.
fn declaring_module(resolved: &ResolvedModule, source_path: &std::path::Path) -> Option<String> {
    resolved
        .imports
        .iter()
        .find(|import| import.resolved_path.as_deref() == Some(source_path))
        .map(|import| import.module.clone())
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
        if let Some(external) = resolved.imported_symbols.get(class_name) {
            queue.extend(
                external
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
fn builtin_member_hover(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    member: &str,
) -> Option<String> {
    let (class, declarations) = builtin_member_declarations(resolved, source, byte_offset, member)?;
    let signatures = declarations
        .iter()
        .map(|declaration| basilisk_stubs::render_stub_signature(declaration))
        .map(|signature| {
            format!(
                "def {}.{rest}",
                class.declaration.name,
                rest = signature.strip_prefix("def ").unwrap_or(&signature)
            )
        })
        .collect();
    let mut card = SymbolCard {
        kind: Some(SymbolKind::Method),
        signatures,
        ..SymbolCard::default()
    };
    card.provenance = class.provenance.hover_label().map(str::to_owned);
    card.source_path = Some(class.source_identity.clone());
    card.render()
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
            access::receiver_type_name(resolved, source, &receiver).or_else(|| {
                // Declared nowhere — a `for` target binds it (GitHub #390).
                let bound =
                    receiver_scope::loop_binding_type(resolved, source, byte_offset, &receiver)?;
                basilisk_checker::class_naming::class_name_of_type(&bound)
            })
        }
        // The receiver is an EXPRESSION, not a name: `s.upper().`, `f(a).`,
        // `xs[0].`. Type it through the shared engine with the names in view
        // bound, since the engine resolves none of its own (GitHub #390).
        _ => expression_receiver_type(resolved, source, receiver_text),
    }
}

/// The class named by an expression receiver, typed through the shared engine.
fn expression_receiver_type(
    resolved: &ResolvedModule,
    source: &str,
    before_dot: &str,
) -> Option<(String, bool)> {
    let expression = receiver_scope::receiver_expression(before_dot)?;
    let scope = receiver_scope::scope_at(resolved, source, before_dot.len());
    let inferred =
        basilisk_checker::expr_type::infer_expression_source_in_scope(expression, &scope);
    basilisk_checker::class_naming::class_name_of_type(&inferred)
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
