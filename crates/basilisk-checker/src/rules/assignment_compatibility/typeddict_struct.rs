//! Implements [assignment_compatibility] from [CHKARCH-DIAG]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//!
//! PEP 705-aware structural assignability for `TypedDict`-to-`TypedDict`
//! assignments. E0014's default `Named`-vs-`Named` comparison is name equality,
//! which false-positives on every structurally-valid cross-name assignment
//! (`v: A = b` where `b: B`). This module reconstructs each `TypedDict`'s
//! effective field schema and applies the PEP 705 read-only / width consistency
//! rules so only genuinely-incompatible assignments fire.

use std::collections::HashMap;

use basilisk_resolver::{strip_typeddict_qualifiers, ClassInfo, ResolvedModule};

use crate::span_util::slice_span;

/// One effective field of a `TypedDict` schema.
pub(super) struct TdField {
    /// Inner value type with `Required`/`NotRequired`/`ReadOnly` stripped.
    ty: String,
    /// Whether the field is required.
    required: bool,
    /// Whether the field is read-only (PEP 705).
    readonly: bool,
}

/// Field-name → field schema for one `TypedDict`.
pub(super) type TdSchema = HashMap<String, TdField>;

/// Class-name → field schema for every `TypedDict` in the module.
pub(super) type TdSchemas = HashMap<String, TdSchema>;

/// Maximum inheritance depth walked before bailing out (cycle guard).
const MAX_DEPTH: u32 = 64;

/// Build the effective field schema of every `TypedDict` class, merging fields
/// inherited from `TypedDict` bases (most-derived declaration wins).
pub(super) fn build_typeddict_schemas(module: &ResolvedModule) -> TdSchemas {
    let by_name: HashMap<&str, &ClassInfo> = module
        .classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| (c.name.as_str(), c))
        .collect();
    by_name
        .keys()
        .map(|&name| {
            let mut schema = TdSchema::new();
            collect_into(name, &by_name, &module.source, &mut schema, 0);
            // Key by lowercase name: `InferredType::from_annotation` lowercases, so
            // the `Named(..)` values looked up against this map are lowercase.
            (name.to_ascii_lowercase(), schema)
        })
        .collect()
}

/// Insert `name`'s own fields then recurse into bases. The first insertion of a
/// field name wins, so the most-derived declaration shadows inherited ones.
fn collect_into(
    name: &str,
    by_name: &HashMap<&str, &ClassInfo>,
    source: &str,
    schema: &mut TdSchema,
    depth: u32,
) {
    let Some(cls) = by_name.get(name) else { return };
    if depth >= MAX_DEPTH {
        return;
    }
    for attr in &cls.attributes {
        let Some(span) = attr.annotation_span else {
            continue;
        };
        let Some(ann) = slice_span(source, span) else {
            continue;
        };
        if schema.contains_key(attr.name.as_str()) {
            continue;
        }
        let ann = ann.trim();
        let _ = schema.insert(
            attr.name.clone(),
            TdField {
                ty: strip_typeddict_qualifiers(ann).trim().to_owned(),
                required: field_required(ann, cls.is_typeddict_total),
                readonly: attr.is_readonly,
            },
        );
    }
    for base in &cls.bases {
        collect_into(base, by_name, source, schema, depth + 1);
    }
}

/// Required-ness from the annotation wrappers, defaulting to the class totality.
/// `NotRequired` is tested first because it contains the substring `Required`.
fn field_required(ann: &str, total: bool) -> bool {
    if ann.contains("NotRequired") {
        false
    } else if ann.contains("Required") {
        true
    } else {
        total
    }
}

/// `true` when a value of `TypedDict` `source` is assignable to `target` under the
/// PEP 705 structural consistency rules (read-only covariance, mutable invariance,
/// required/optional width). Keys present only in `source` are allowed (width
/// subtyping); each key of `target` must be satisfied by `source`.
pub(super) fn typeddict_assignable(source: &TdSchema, target: &TdSchema) -> bool {
    target.iter().all(|(key, t)| match source.get(key) {
        // A missing key is allowed only for a read-only, not-required, top-typed
        // target item (`ReadOnly[NotRequired[object]]`).
        None => t.readonly && !t.required && is_top_type(&t.ty),
        Some(s) => field_assignable(s, t),
    })
}

/// Per-field assignability of a source field `s` to a target field `t`.
fn field_assignable(s: &TdField, t: &TdField) -> bool {
    let value_ok = if t.readonly {
        // Read-only target: covariant — the source value need only be consistent.
        types_consistent(&s.ty, &t.ty)
    } else {
        // Mutable target: the source must also be mutable, with an invariant type.
        !s.readonly && s.ty == t.ty
    };
    // A required target key must be required in the source; a mutable, optional
    // target key additionally forbids a required source (it could not be deleted).
    let required_ok = (!t.required || s.required) && (t.required || t.readonly || !s.required);
    value_ok && required_ok
}

/// Whether `actual` is consistent with a read-only `expected` value type: an exact
/// match, or `expected` is the top type (`object`/`Any`).
fn types_consistent(actual: &str, expected: &str) -> bool {
    actual == expected || is_top_type(expected)
}

/// `true` for the top value type — `object` or `Any`.
fn is_top_type(ty: &str) -> bool {
    ty == "object" || ty == "Any"
}
