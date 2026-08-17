//! Implements [CHKARCH-ARCH-PIPELINE] and the identity-keyed variable→class
//! association of [RESOLV-CANONICAL-BINDING]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//! Typeddict visitor functions.
//!
//! Every schema lookup in this family is keyed on the DEFINITION SITE of the
//! class ([`ClassInfo::name_span`]), never on a spelling. The deleted
//! predecessor joined `m: Movie` to `class Movie` by comparing the
//! annotation's characters to the class's characters, so an aliased or dotted
//! annotation validated nothing and two same-named classes shared one schema.
//! Here the annotation expression resolves through the module's
//! [`BindingTable`] to the `class` statement it denotes, and the schema map is
//! keyed by that statement's name-token span — the same contract
//! [`crate::scope::ClassGraph::at`] answers.

use ruff_python_ast::{Expr, Stmt, StmtAssign};
use ruff_text_size::TextRange;

use basilisk_canonical::{BindingTable, TypingForm};

use crate::scope::{ClassInfo, Span, TypedDictKeyViolation, TypedDictKeyViolationKind};

use super::class_info_ext::expr_simple_name;
use super::core::{check_td_stmts, text_range_to_span};

/// The effective (post-inheritance) schema of one `TypedDict` class.
///
/// [`Self::class_name`] is a RENDERING carried for diagnostic **message** text
/// only; every lookup that reaches a `TdSchema` travelled through a
/// [`Span`] key.
pub(super) struct TdSchema<'a> {
    /// The class's declared name — diagnostic message text, never a key.
    pub class_name: &'a str,
    /// Every field of the effective schema, most-derived declaration first.
    pub all_fields: Vec<&'a str>,
    /// Field name → raw annotation text of the most-derived declaration.
    pub field_types: std::collections::HashMap<&'a str, String>,
    /// `total=` in force at the class's own definition.
    pub is_total: bool,
    /// PEP 728 `extra_items=` anywhere in the resolved chain.
    pub has_extra_items: bool,
}

/// `TypedDict` schemas keyed by definition site ([`ClassInfo::name_span`]).
pub(super) type TdFieldMap<'a> = std::collections::HashMap<Span, TdSchema<'a>>;

/// The definition site of the local class an annotation refers to.
///
/// Resolves every lawful route to a class this module defines
/// ([ASTREBUILD-LAW]): a direct reference (`m: Movie`), a subscripted one
/// (`m: Movie[int]`), an assignment alias (`Alias = Movie; m: Alias`), and a
/// quoted forward reference (`m: "Movie"`, resolved against the module's
/// final namespace as PEP 484 prescribes). `None` means "not a class this
/// module defines" — an import, a builtin, a rebound name — and every caller
/// treats it as abstention.
pub(super) fn annotation_local_class(
    bindings: &BindingTable,
    annotation: &Expr,
) -> Option<TextRange> {
    match annotation {
        Expr::StringLiteral(quoted) => {
            bindings.local_class_of_quoted_annotation(quoted.value.to_str())
        }
        _ => bindings.local_class_definition(annotation),
    }
}

/// The definition site of the local class a `**kwargs` annotation unpacks.
///
/// PEP 692: `**kwargs: Unpack[Movie]` types the keyword mapping with
/// `Movie`'s schema. The `Unpack` head resolves through the bindings — an
/// aliased `Unpack` unwraps, a module-local `class Unpack` does not — and the
/// element resolves like any other annotation, so `Unpack["Movie"]` and
/// `Unpack[MovieAlias]` both reach the class.
pub(super) fn kwargs_unpacked_local_class(
    bindings: &BindingTable,
    annotation: &Expr,
) -> Option<TextRange> {
    let element = bindings.subscript_element(annotation, TypingForm::Unpack)?;
    annotation_local_class(bindings, element)
}

/// Collect `TypedDict` key/value violations from module-level statements and function bodies.
///
/// Detects:
/// - Subscript assignments with invalid keys: `movie["director"] = "Ridley Scott"`
/// - Annotated dict literal assignments with invalid or missing keys
/// - Regular dict assignments to `TypedDict` variables with wrong keys
/// - Subscript read access with invalid keys: `print(movie["unknown"])`
/// - Disallowed method calls: `movie.clear()`
/// - Delete operations on required `TypedDict` keys: `del movie["name"]`
pub(super) fn collect_typeddict_key_violations<'a>(
    bindings: &BindingTable,
    stmts: &[Stmt],
    classes: &'a [ClassInfo],
    source: &'a str,
) -> Vec<TypedDictKeyViolation> {
    // Membership and inheritance come from the resolved hierarchy — bases
    // resolved through the module's bindings, keyed on definition site — and
    // the schema map is keyed the same way, so the variable→class association
    // below joins on identity end to end.
    let graph = crate::scope::ClassGraph::new(classes);

    let typeddict_fields: TdFieldMap<'a> = graph
        .typed_dicts()
        .into_iter()
        .map(|c| {
            // Merge own + inherited fields so transitive subclasses
            // (`class Album(NamedDict): ...`) carry the full schema and the
            // most-derived declaration of each redeclared field.
            let effective = super::typeddict_schema::effective_fields(c, &graph, source);
            let all_fields: Vec<&str> = effective.iter().map(|f| f.name).collect();
            let field_types: std::collections::HashMap<&str, String> = effective
                .iter()
                .filter_map(|f| f.annotation.map(|ann| (f.name, ann.to_owned())))
                .collect();
            let has_extra_items = graph.has_extra_items(c);
            (
                c.name_span,
                TdSchema {
                    class_name: c.name.as_str(),
                    all_fields,
                    field_types,
                    is_total: c.is_typeddict_total,
                    has_extra_items,
                },
            )
        })
        .collect();

    if typeddict_fields.is_empty() {
        return Vec::new();
    }

    let var_type = td_var_type_from_stmts(bindings, stmts, &typeddict_fields);
    let mut out = Vec::new();
    check_td_stmts(bindings, &typeddict_fields, &var_type, stmts, &mut out);
    out
}

/// Associate annotated variables with the `TypedDict` each annotation denotes.
///
/// The returned map's values are definition sites into `fields`; the keys are
/// the variable names being tracked through the enclosing scope's walk. Only
/// annotations that resolve — through the bindings, alias hops and quoting
/// included — to a class in `fields` are recorded; everything else is
/// abstention, never a guess.
pub(super) fn td_var_type_from_stmts(
    bindings: &BindingTable,
    stmts: &[Stmt],
    fields: &TdFieldMap<'_>,
) -> std::collections::HashMap<String, Span> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(var_name) = ann.target.as_ref() else {
            continue;
        };
        let Some(site) = annotation_local_class(bindings, &ann.annotation) else {
            continue;
        };
        let span = text_range_to_span(site);
        if fields.contains_key(&span) {
            let _ = map.insert(var_name.id.to_string(), span);
        }
    }
    map
}

/// Recursively check statements for `TypedDict` violations.
pub(super) fn td_check_subscript_assign(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, Span>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Expr::Subscript(sub) = target else {
            continue;
        };
        let Some(var_name) = expr_simple_name(&sub.value) else {
            continue;
        };
        let Some(site) = var_type.get(&var_name) else {
            continue;
        };
        let Some(schema) = fields.get(site) else {
            continue;
        };
        let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
            continue;
        };
        let key = key_str.value.to_string();
        if !schema.all_fields.contains(&key.as_str()) && !schema.has_extra_items {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: schema.class_name.to_owned(),
                kind: TypedDictKeyViolationKind::InvalidSubscriptKey { key },
            });
        }
    }
}
