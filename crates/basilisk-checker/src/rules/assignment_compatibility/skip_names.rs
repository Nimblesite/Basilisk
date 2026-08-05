//! Implements the false-positive skip environment of [TYPEINF-VARS-ANNOTATED].
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-ANNOTATED
//!
//! Everything `assignment_compatibility` must NOT flag: name sets whose
//! declared types it cannot evaluate (`TypedDict`s, aliases), the alias/schema
//! environments used by rescue checks, and `if not TYPE_CHECKING:` blocks
//! (PEP 484 excludes them from type checking entirely).

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;
use crate::types::InferredType;

use super::{alias_match, enum_expand, typeddict_struct, CODE};

/// Names that E0014 must skip to avoid false positives.
pub(super) struct SkipNames {
    /// `TypedDict` class names (lowercase).
    pub(super) typeddict: std::collections::HashSet<String>,
    /// `TypedDict` classes declaring `extra_items=` (PEP 728, lowercase).
    pub(super) typeddict_extra_items: std::collections::HashSet<String>,
    /// Legacy value aliases — `Name = Union[...]` or a concrete container such
    /// as `Name = dict[K, V]` (lowercase → definition), used for alias-expanded
    /// value matching.
    pub(super) value_aliases: std::collections::HashMap<String, InferredType>,
    /// Generic (`TypeVar`-parameterised) value aliases such as
    /// `G = list["G[T]" | T]`, keyed by lowercase name. Used to validate
    /// literal assignments against a specialised recursive alias (`G[str]`).
    pub(super) generic_aliases: std::collections::HashMap<String, alias_match::GenericAlias>,
    /// Effective field schemas (class name → fields) for every `TypedDict`,
    /// used for PEP 705 structural assignability of `TypedDict`-to-`TypedDict`
    /// assignments instead of name equality.
    pub(super) typeddict_schemas: typeddict_struct::TdSchemas,
    /// Enum class name → member names (lowercase), for the enum literal
    /// expansion equivalence ([TYPEINF-SUBTYPING-UNION]).
    pub(super) enum_members: enum_expand::EnumMembers,
}

impl SkipNames {
    /// Build the full skip environment for a module.
    pub(super) fn collect(
        module: &ResolvedModule,
        resolver: &crate::annotation::AnnotationResolver<'_>,
    ) -> Self {
        Self {
            typeddict: collect_typeddict_names(module),
            typeddict_extra_items: collect_extra_items_typeddict_names(module),
            value_aliases: alias_match::collect_value_aliases(module, resolver),
            generic_aliases: alias_match::collect_generic_aliases(module),
            typeddict_schemas: typeddict_struct::build_typeddict_schemas(module),
            enum_members: enum_expand::collect_enum_member_sets(module),
        }
    }
}

/// Collect names of `TypedDict` classes defined in this module.
///
/// `assignment_compatibility` cannot do structural field-level type checking on `TypedDict`
/// subclasses, so dict literal assignments to `TypedDict` annotations are
/// skipped to avoid false positives.
fn collect_typeddict_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    // Recognise transitive TypedDict subclasses (`class Album(NamedDict): ...`),
    // not just classes that name `TypedDict` directly. Otherwise E0014 stops
    // skipping their dict-literal assignments and false-positives on every valid
    // `album: Album = {...}` whose base — not the leaf — is the TypedDict.
    let mut names: std::collections::HashSet<String> =
        basilisk_resolver::transitive_typeddict_names(&module.classes)
            .into_iter()
            .map(str::to_ascii_lowercase)
            .collect();

    // Include functional-form TypedDicts: `Name = TypedDict("Name", {...})`.
    for td_call in &module.typeddict_calls {
        let _ = names.insert(td_call.lhs_name.to_ascii_lowercase());
    }

    names
}

/// Names of `TypedDict` classes declaring `extra_items=` (lowercase).
///
/// Such `TypedDict`s may be assignable to `dict[str, VT]` (PEP 728), which
/// E0014's name-level comparison cannot evaluate — those assignments are
/// skipped rather than flagged.
fn collect_extra_items_typeddict_names(
    module: &ResolvedModule,
) -> std::collections::HashSet<String> {
    module
        .classes
        .iter()
        .filter(|cls| cls.class_keywords.iter().any(|kw| kw == "extra_items"))
        .map(|cls| cls.name.to_ascii_lowercase())
        .collect()
}

/// Remove E0014 diagnostics inside `if not TYPE_CHECKING:` blocks — that code
/// is explicitly excluded from type checking (PEP 484).
pub(super) fn drop_unchecked_block_diagnostics(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    let Some(parsed) = crate::rules::shared::parse_module(module) else {
        return;
    };
    let blocks: Vec<(u32, u32)> = parsed
        .ast
        .body
        .iter()
        .filter_map(|stmt| {
            let ruff_python_ast::Stmt::If(if_stmt) = stmt else {
                return None;
            };
            let ruff_python_ast::Expr::UnaryOp(unary) = if_stmt.test.as_ref() else {
                return None;
            };
            let is_not_type_checking = unary.op == ruff_python_ast::UnaryOp::Not
                && matches!(
                    unary.operand.as_ref(),
                    ruff_python_ast::Expr::Name(n) if n.id.as_str() == "TYPE_CHECKING"
                );
            is_not_type_checking.then(|| {
                let range = if_stmt.range();
                (range.start().to_u32(), range.end().to_u32())
            })
        })
        .collect();
    if blocks.is_empty() {
        return;
    }
    diagnostics.retain(|diag| {
        diag.code.code != CODE.code
            || !blocks
                .iter()
                .any(|&(start, end)| diag.span.start >= start && diag.span.end <= end)
    });
}
