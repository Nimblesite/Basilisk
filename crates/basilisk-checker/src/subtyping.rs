//! Class hierarchy and structural subtyping engine.
//!
//! Provides nominal subtyping via MRO resolution, protocol structural
//! subtyping (PEP 544), generic variance checking, `TypedDict` structural
//! subtyping, and callable subtyping extensions.
//!
//! This module replaces the simple `Named`-to-`Named` string comparison
//! in `is_assignable_to()` with full hierarchy-aware subtype checks.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::span_util::slice_span;
use crate::types::InferredType;

/// Precomputed subtyping context built from a resolved module.
///
/// Caches MRO chains, protocol member tables, and class metadata to
/// enable efficient subtype queries during rule checking.
pub struct SubtypeContext<'a> {
    /// Class name -> ordered MRO (C3 linearization).
    mro_cache: HashMap<&'a str, Vec<&'a str>>,
    /// Class name -> `ClassInfo` reference.
    class_map: HashMap<&'a str, &'a ClassInfo>,
    /// Class name -> method names with return type annotations.
    method_returns: HashMap<(&'a str, &'a str), Option<String>>,
    /// Class name -> attribute name -> annotation text.
    attr_types: HashMap<(&'a str, &'a str), Option<String>>,
    /// Protocol name -> required members.
    protocol_members: HashMap<&'a str, Vec<ProtocolMember>>,
    /// Class name -> `is_protocol` flag.
    protocol_names: HashMap<&'a str, bool>,
    /// `TypedDict` name -> field info.
    typeddict_fields: HashMap<&'a str, Vec<TypedDictField>>,
}

/// A required member of a protocol.
#[derive(Debug, Clone)]
struct ProtocolMember {
    /// Member name.
    name: String,
    /// What kind of member this is.
    kind: MemberKind,
    /// Type annotation text (for attribute type checking).
    type_text: Option<String>,
}

/// The kind of a protocol member.
#[derive(Debug, Clone, PartialEq)]
enum MemberKind {
    /// A regular method.
    Method,
    /// A read-only property (`@property` without setter).
    Property,
    /// A plain attribute annotation.
    Attribute,
}

/// A field in a `TypedDict`.
#[derive(Debug, Clone)]
struct TypedDictField {
    /// Field name.
    name: String,
    /// Type annotation text.
    type_text: Option<String>,
    /// Whether this field is required.
    required: bool,
    /// Whether this field is read-only.
    read_only: bool,
}

/// Hardcoded MRO chains for builtin types.
///
/// These represent the nominal class hierarchy for Python builtins
/// that are not defined in user code.
pub(crate) fn builtin_mro(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "bool" => Some(&["bool", "int", "float", "complex", "object"]),
        "int" => Some(&["int", "float", "complex", "object"]),
        "float" => Some(&["float", "complex", "object"]),
        "complex" => Some(&["complex", "object"]),
        "str" => Some(&["str", "Sequence", "Hashable", "object"]),
        "bytes" => Some(&["bytes", "Sequence", "Hashable", "object"]),
        "bytearray" => Some(&[
            "bytearray",
            "MutableSequence",
            "Sequence",
            "Hashable",
            "object",
        ]),
        "list" => Some(&[
            "list",
            "MutableSequence",
            "Sequence",
            "Reversible",
            "Collection",
            "Iterable",
            "object",
        ]),
        "tuple" => Some(&["tuple", "Sequence", "Hashable", "object"]),
        "dict" => Some(&[
            "dict",
            "MutableMapping",
            "Mapping",
            "Collection",
            "Iterable",
            "object",
        ]),
        "set" => Some(&[
            "set",
            "MutableSet",
            "AbstractSet",
            "Collection",
            "Iterable",
            "object",
        ]),
        "frozenset" => Some(&[
            "frozenset",
            "AbstractSet",
            "Collection",
            "Iterable",
            "object",
        ]),
        "range" => Some(&["range", "Sequence", "object"]),
        "memoryview" => Some(&["memoryview", "Sequence", "object"]),
        "NoneType" => Some(&["NoneType", "Hashable", "object"]),
        "type" => Some(&["type", "object"]),
        "object" => Some(&["object"]),
        // Abstract base classes from collections.abc / typing.
        "Iterable" | "iterable" => Some(&["Iterable", "object"]),
        "Iterator" | "iterator" => Some(&["Iterator", "Iterable", "object"]),
        "Generator" | "generator" => Some(&["Generator", "Iterator", "Iterable", "object"]),
        "Reversible" | "reversible" => Some(&["Reversible", "Iterable", "object"]),
        "Collection" | "collection" => Some(&["Collection", "Iterable", "object"]),
        "Sequence" | "sequence" => {
            Some(&["Sequence", "Reversible", "Collection", "Iterable", "object"])
        }
        "MutableSequence" | "mutablesequence" => Some(&[
            "MutableSequence",
            "Sequence",
            "Reversible",
            "Collection",
            "Iterable",
            "object",
        ]),
        "Mapping" | "mapping" => Some(&["Mapping", "Collection", "Iterable", "object"]),
        "MutableMapping" | "mutablemapping" => Some(&[
            "MutableMapping",
            "Mapping",
            "Collection",
            "Iterable",
            "object",
        ]),
        "AbstractSet" | "abstractset" => Some(&["AbstractSet", "Collection", "Iterable", "object"]),
        "MutableSet" | "mutableset" => Some(&[
            "MutableSet",
            "AbstractSet",
            "Collection",
            "Iterable",
            "object",
        ]),
        "Hashable" | "hashable" => Some(&["Hashable", "object"]),
        "Sized" | "sized" => Some(&["Sized", "object"]),
        "Callable" | "callable" => Some(&["Callable", "object"]),
        "Awaitable" | "awaitable" => Some(&["Awaitable", "object"]),
        "Coroutine" | "coroutine" => Some(&["Coroutine", "Awaitable", "object"]),
        "AsyncIterable" | "asynciterable" => Some(&["AsyncIterable", "object"]),
        "AsyncIterator" | "asynciterator" => Some(&["AsyncIterator", "AsyncIterable", "object"]),
        "SupportsInt" | "supportsint" => Some(&["SupportsInt", "object"]),
        "SupportsFloat" | "supportsfloat" => Some(&["SupportsFloat", "object"]),
        "SupportsComplex" | "supportscomplex" => Some(&["SupportsComplex", "object"]),
        "SupportsBytes" | "supportsbytes" => Some(&["SupportsBytes", "object"]),
        "SupportsAbs" | "supportsabs" => Some(&["SupportsAbs", "object"]),
        "SupportsRound" | "supportsround" => Some(&["SupportsRound", "object"]),
        _ => None,
    }
}

/// Protocol base class names that should be skipped when collecting
/// protocol members (they don't contribute required members).
const PROTOCOL_META_BASES: &[&str] = &[
    "Protocol",
    "typing.Protocol",
    "typing_extensions.Protocol",
    "Generic",
    "typing.Generic",
    "object",
];

impl<'a> SubtypeContext<'a> {
    /// Build a subtype context from a resolved module.
    #[must_use]
    pub fn from_module(module: &'a ResolvedModule) -> Self {
        let mut ctx = Self {
            mro_cache: HashMap::new(),
            class_map: HashMap::new(),
            method_returns: HashMap::new(),
            attr_types: HashMap::new(),
            protocol_members: HashMap::new(),
            protocol_names: HashMap::new(),
            typeddict_fields: HashMap::new(),
        };

        // Index all classes.
        for class in &module.classes {
            let _ = ctx.class_map.insert(class.name.as_str(), class);

            // Detect protocols.
            let is_protocol = class.bases.iter().any(|b| {
                matches!(
                    b.as_str(),
                    "Protocol" | "typing.Protocol" | "typing_extensions.Protocol"
                )
            });
            let _ = ctx.protocol_names.insert(class.name.as_str(), is_protocol);

            // Index attributes.
            for attr in &class.attributes {
                let type_text = attr
                    .annotation_span
                    .and_then(|sp| slice_span(&module.source, sp))
                    .map(|s| s.trim().to_owned());
                let _ = ctx
                    .attr_types
                    .insert((class.name.as_str(), attr.name.as_str()), type_text);
            }

            // Index TypedDict fields.
            if class.is_typed_dict {
                let fields: Vec<TypedDictField> = class
                    .attributes
                    .iter()
                    .map(|attr| {
                        let type_text = attr
                            .annotation_span
                            .and_then(|sp| slice_span(&module.source, sp))
                            .map(|s| s.trim().to_owned());
                        let (required, read_only) = parse_typeddict_field_flags(
                            type_text.as_deref(),
                            class.is_typeddict_total,
                        );
                        TypedDictField {
                            name: attr.name.clone(),
                            type_text,
                            required,
                            read_only,
                        }
                    })
                    .collect();
                let _ = ctx.typeddict_fields.insert(class.name.as_str(), fields);
            }

            // Collect protocol members.
            if is_protocol {
                let members = collect_protocol_members(class, &module.source, &module.functions);
                let _ = ctx.protocol_members.insert(class.name.as_str(), members);
            }
        }

        // Index method return types.
        for func in &module.functions {
            if let Some(class_name) = &func.class_name {
                let ret_text = func
                    .return_annotation_span
                    .and_then(|sp| slice_span(&module.source, sp))
                    .map(|s| s.trim().to_owned());
                let _ = ctx
                    .method_returns
                    .insert((class_name.as_str(), func.name.as_str()), ret_text);
            }
        }

        // Compute MRO for all user-defined classes.
        let class_names: Vec<&str> = ctx.class_map.keys().copied().collect();
        for name in class_names {
            let _ = ctx.compute_mro(name);
        }

        ctx
    }

    /// Compute and cache the MRO for a class via C3 linearization.
    fn compute_mro(&mut self, class_name: &'a str) -> Vec<&'a str> {
        // Check cache first.
        if let Some(cached) = self.mro_cache.get(class_name) {
            return cached.clone();
        }

        // Check builtin MRO.
        if let Some(builtin) = builtin_mro(class_name) {
            let mro: Vec<&str> = builtin.to_vec();
            let _ = self.mro_cache.insert(class_name, mro.clone());
            return mro;
        }

        // Look up class info.
        let Some(class) = self.class_map.get(class_name) else {
            // Unknown class — assume MRO is just [self, object].
            let mro = vec![class_name, "object"];
            let _ = self.mro_cache.insert(class_name, mro.clone());
            return mro;
        };

        // Collect base MROs.
        let bases: Vec<String> = class
            .bases
            .iter()
            .filter(|b| !PROTOCOL_META_BASES.contains(&b.as_str()))
            .cloned()
            .collect();

        // Simple linearization: class + flatten(base MROs) + object.
        // Full C3 is complex; this handles single-inheritance and simple
        // multiple-inheritance correctly, which covers most Python code.
        let mut mro: Vec<&'a str> = vec![class_name];
        for base_name in &bases {
            // Recursively compute base MRO (need to handle the borrow carefully).
            let base_mro = if let Some(builtin) = builtin_mro(base_name) {
                builtin.to_vec()
            } else if let Some(cached) = self.mro_cache.get(base_name.as_str()) {
                cached.clone()
            } else {
                // For user-defined bases, build a simple chain.
                self.build_simple_mro(base_name)
            };

            for ancestor in base_mro {
                if !mro.contains(&ancestor) {
                    mro.push(ancestor);
                }
            }
        }

        // Ensure `object` is at the end.
        if !mro.contains(&"object") {
            mro.push("object");
        }

        let _ = self.mro_cache.insert(class_name, mro.clone());
        mro
    }

    /// Build a simple MRO chain for a user-defined class without full C3.
    fn build_simple_mro(&self, class_name: &str) -> Vec<&'a str> {
        let Some(class) = self.class_map.get(class_name) else {
            return vec!["object"];
        };

        let mut mro = vec![class.name.as_str()];
        for base in &class.bases {
            if PROTOCOL_META_BASES.contains(&base.as_str()) {
                continue;
            }
            if let Some(builtin) = builtin_mro(base) {
                for ancestor in builtin {
                    if !mro.contains(ancestor) {
                        mro.push(ancestor);
                    }
                }
            } else if let Some(base_class) = self.class_map.get(base.as_str()) {
                if !mro.contains(&base_class.name.as_str()) {
                    mro.push(base_class.name.as_str());
                }
            }
        }
        if !mro.contains(&"object") {
            mro.push("object");
        }
        mro
    }

    /// Check if `source_name` is a subtype of `target_name`.
    ///
    /// This is the main entry point for subtype queries. It checks:
    /// 1. Exact name match (including generic base name match)
    /// 2. Nominal subtyping via MRO
    /// 3. Protocol structural subtyping
    /// 4. `TypedDict` structural subtyping
    /// 5. Builtin special cases
    #[must_use]
    pub fn is_subtype(&self, source_name: &str, target_name: &str) -> bool {
        let source_base = source_name.split('[').next().unwrap_or(source_name);
        let target_base = target_name.split('[').next().unwrap_or(target_name);

        // Same base name — compatible (preserves existing behaviour).
        // Case-insensitive because `from_annotation` lowercases names.
        if source_base.eq_ignore_ascii_case(target_base) {
            return true;
        }

        // `object` is the universal supertype.
        if target_base.eq_ignore_ascii_case("object") {
            return true;
        }

        // `Never` is the bottom type — subtype of everything.
        if source_base == "Never" {
            return true;
        }

        // Resolve lowercased names to their original-case equivalents
        // for cache/map lookups (from_annotation lowercases everything).
        let source_resolved = self.resolve_class_name(source_base);
        let target_resolved = self.resolve_class_name(target_base);
        let source_lookup = source_resolved.as_deref().unwrap_or(source_base);
        let target_lookup = target_resolved.as_deref().unwrap_or(target_base);

        // Nominal subtyping via MRO.
        if self.is_nominal_subtype(source_lookup, target_lookup) {
            return true;
        }

        // Protocol structural subtyping.
        if self.is_protocol_subtype(source_lookup, target_lookup) {
            return true;
        }

        // TypedDict structural subtyping.
        if self.is_typeddict_subtype(source_lookup, target_lookup) {
            return true;
        }

        // Builtin coercion: bool -> int -> float -> complex.
        if is_builtin_numeric_subtype(source_base, target_base) {
            return true;
        }

        // Container covariance for common abstract types.
        if is_abstract_container_subtype(source_name, target_name) {
            return true;
        }

        false
    }

    /// Resolve a (possibly lowercased) class name to its original-case form.
    ///
    /// `from_annotation` lowercases all names, but our maps use original case.
    /// This method finds the canonical class name via case-insensitive search.
    fn resolve_class_name(&self, name: &str) -> Option<String> {
        // Fast path: exact match.
        if self.class_map.contains_key(name) {
            return Some(name.to_owned());
        }
        // Case-insensitive fallback.
        for &key in self.class_map.keys() {
            if key.eq_ignore_ascii_case(name) {
                return Some(key.to_owned());
            }
        }
        None
    }

    /// Returns `true` if `name` is a known class (user-defined or builtin).
    ///
    /// Names that are NOT known include type aliases, imported names not
    /// defined in this module, and forward references we cannot resolve.
    #[must_use]
    pub fn is_name_known(&self, name: &str) -> bool {
        self.resolve_class_name(name).is_some()
            || builtin_mro(name).is_some()
            || is_well_known_type(name)
    }

    /// Returns `true` if `name` is a known `TypedDict` class (including
    /// transitive inheritance from a `TypedDict`).
    #[must_use]
    pub fn is_typeddict(&self, name: &str) -> bool {
        if let Some(resolved) = self.resolve_class_name(name) {
            if self.typeddict_fields.contains_key(resolved.as_str()) {
                return true;
            }
            // Check if any ancestor in the MRO is a TypedDict.
            if let Some(mro) = self.mro_cache.get(resolved.as_str()) {
                return mro
                    .iter()
                    .any(|&base| self.typeddict_fields.contains_key(base));
            }
        }
        false
    }

    /// Returns `true` if `name` is a known protocol class.
    #[must_use]
    pub fn is_protocol(&self, name: &str) -> bool {
        if let Some(resolved) = self.resolve_class_name(name) {
            return self.protocol_names.get(resolved.as_str()) == Some(&true);
        }
        false
    }

    /// Check nominal subtyping: is `source` in `target`'s MRO?
    fn is_nominal_subtype(&self, source: &str, target: &str) -> bool {
        // Check source's MRO for target (case-insensitive because
        // `from_annotation` lowercases names while `builtin_mro` uses
        // mixed-case entries like "Iterable", "Hashable", etc.).
        if let Some(mro) = self.mro_cache.get(source) {
            if mro.iter().any(|m| m.eq_ignore_ascii_case(target)) {
                return true;
            }
        }

        // Check builtin MRO.
        if let Some(mro) = builtin_mro(source) {
            if mro.iter().any(|m| m.eq_ignore_ascii_case(target)) {
                return true;
            }
        }

        false
    }

    /// Check protocol structural subtyping.
    fn is_protocol_subtype(&self, source: &str, target: &str) -> bool {
        // Target must be a protocol.
        let Some(true) = self.protocol_names.get(target) else {
            return false;
        };

        let Some(required_members) = self.protocol_members.get(target) else {
            // Protocol with no members — everything satisfies it.
            return true;
        };

        if required_members.is_empty() {
            return true;
        }

        // Check if source has all required members.
        required_members
            .iter()
            .all(|member| self.source_has_member(source, member))
    }

    /// Check if a source class has a member matching a protocol requirement.
    fn source_has_member(&self, source: &str, required: &ProtocolMember) -> bool {
        match required.kind {
            MemberKind::Method => {
                // Check if source has a method with this name.
                if let Some(class) = self.class_map.get(source) {
                    if class.method_names.contains(&required.name) {
                        return true;
                    }
                    // Check inherited methods via MRO.
                    if let Some(mro) = self.mro_cache.get(source) {
                        for &ancestor in mro.iter().skip(1) {
                            if let Some(ancestor_class) = self.class_map.get(ancestor) {
                                if ancestor_class.method_names.contains(&required.name) {
                                    return true;
                                }
                            }
                        }
                    }
                }
                // Check builtins for common methods.
                has_builtin_method(source, &required.name)
            }
            MemberKind::Property | MemberKind::Attribute => {
                // Find the source's type annotation for this member.
                let source_type_text = self.find_member_type(source, &required.name);

                // If the source doesn't have this member at all, fail.
                if source_type_text.is_none() && !self.has_member_by_name(source, &required.name) {
                    return false;
                }

                // If the protocol specifies a type, check compatibility.
                if let (Some(req_type), Some(src_type)) = (&required.type_text, &source_type_text) {
                    let req_inferred = InferredType::from_annotation(req_type);
                    let src_inferred = InferredType::from_annotation(src_type);
                    // Mutable attributes require invariance.
                    if required.kind == MemberKind::Attribute {
                        return src_inferred.is_assignable_to(&req_inferred)
                            && req_inferred.is_assignable_to(&src_inferred);
                    }
                }

                true
            }
        }
    }

    /// Find the type annotation text for a member of a source class.
    fn find_member_type(&self, source: &str, member_name: &str) -> Option<String> {
        // Check direct attributes.
        if let Some(type_text) = self.attr_types.get(&(source, member_name)) {
            return type_text.clone();
        }
        // Check inherited attributes via MRO.
        if let Some(mro) = self.mro_cache.get(source) {
            for &ancestor in mro.iter().skip(1) {
                if let Some(type_text) = self.attr_types.get(&(ancestor, member_name)) {
                    return type_text.clone();
                }
            }
        }
        None
    }

    /// Check if a source class has a member by name (attribute or method).
    fn has_member_by_name(&self, source: &str, member_name: &str) -> bool {
        if self.attr_types.contains_key(&(source, member_name)) {
            return true;
        }
        if let Some(class) = self.class_map.get(source) {
            if class.method_names.iter().any(|m| m == member_name) {
                return true;
            }
        }
        if let Some(mro) = self.mro_cache.get(source) {
            for &ancestor in mro.iter().skip(1) {
                if self.attr_types.contains_key(&(ancestor, member_name)) {
                    return true;
                }
                if let Some(ancestor_class) = self.class_map.get(ancestor) {
                    if ancestor_class.method_names.iter().any(|m| m == member_name) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check `TypedDict` structural subtyping.
    fn is_typeddict_subtype(&self, source: &str, target: &str) -> bool {
        let Some(target_fields) = self.typeddict_fields.get(target) else {
            return false;
        };
        let Some(source_fields) = self.typeddict_fields.get(source) else {
            return false;
        };

        // Every field in the target must exist in the source with compatible type.
        // Non-required read-only fields with top value type (`object`) may be
        // absent from the source (PEP 705).
        for target_field in target_fields {
            let source_field = source_fields.iter().find(|sf| sf.name == target_field.name);
            if source_field.is_none() {
                // Missing field: allowed only if target field is
                // read-only + not required + top value type.
                if target_field.read_only && !target_field.required {
                    continue;
                }
                return false;
            }
            // Type compatibility check: if both have type annotations, compare them.
            // Strip ReadOnly/Required/NotRequired wrappers before comparison.
            if let (Some(target_type), Some(source_type)) = (
                &target_field.type_text,
                source_field.and_then(|sf| sf.type_text.as_ref()),
            ) {
                let target_inner = strip_typeddict_wrappers(target_type);
                let source_inner = strip_typeddict_wrappers(source_type);
                let target_inferred = InferredType::from_annotation(target_inner);
                let source_inferred = InferredType::from_annotation(source_inner);

                // A read-only source field cannot satisfy a mutable target
                // field — the target could mutate the value.
                let source_is_read_only = source_field.is_some_and(|sf| sf.read_only);
                if !target_field.read_only && source_is_read_only {
                    return false;
                }

                let source_is_required = source_field.is_some_and(|sf| sf.required);

                // For each required key in B (target), the corresponding
                // key must be required in A (source).
                if target_field.required && !source_is_required {
                    return false;
                }

                // For each non-required mutable key in B (target), the
                // corresponding key must NOT be required in A (source).
                if !target_field.required && !target_field.read_only && source_is_required {
                    return false;
                }

                if target_field.read_only {
                    // ReadOnly fields are covariant.
                    if !source_inferred.is_assignable_to(&target_inferred) {
                        return false;
                    }
                } else {
                    // Mutable fields are invariant (bidirectional check).
                    if !source_inferred.is_assignable_to(&target_inferred)
                        || !target_inferred.is_assignable_to(&source_inferred)
                    {
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// Check abstract container subtyping (e.g. `list` <: `Sequence`).
fn is_abstract_container_subtype(source: &str, target: &str) -> bool {
    let source_base = source.split('[').next().unwrap_or(source);
    let target_base = target.split('[').next().unwrap_or(target);

    // Check if source_base's builtin MRO contains target_base
    // (case-insensitive — `from_annotation` lowercases names).
    if let Some(mro) = builtin_mro(source_base) {
        if mro.iter().any(|m| m.eq_ignore_ascii_case(target_base)) {
            return true;
        }
    }

    false
}

/// Check if source is a numeric subtype of target via the builtin
/// bool -> int -> float -> complex widening chain.
fn is_builtin_numeric_subtype(source: &str, target: &str) -> bool {
    matches!(
        (source, target),
        ("bool", "int" | "float" | "complex") | ("int", "float" | "complex") | ("float", "complex")
    )
}

/// Check if a name is a well-known `typing` / `collections.abc` type.
///
/// These types are always available via import and should be treated as
/// "known" even when not defined in the current module.  This prevents
/// `has_unresolvable_named` from suppressing diagnostics involving
/// common abstract types like `Iterable`, `Hashable`, `Sequence`, etc.
fn is_well_known_type(name: &str) -> bool {
    // Case-insensitive: from_annotation lowercases everything.
    matches!(
        name,
        "iterable"
            | "iterator"
            | "reversible"
            | "collection"
            | "sequence"
            | "mutablesequence"
            | "mapping"
            | "mutablemapping"
            | "abstractset"
            | "mutableset"
            | "hashable"
            | "sized"
            | "callable"
            | "awaitable"
            | "coroutine"
            | "asynciterable"
            | "asynciterator"
            | "asyncgenerator"
            | "generator"
            | "container"
            | "supportsint"
            | "supportsfloat"
            | "supportscomplex"
            | "supportsbytes"
            | "supportsabs"
            | "supportsround"
            | "contextmanager"
            | "asynccontextmanager"
            | "pattern"
            | "match"
            | "io"
            | "path"
            | "classvar"
    )
}

/// Check if a builtin type has a specific method.
fn has_builtin_method(type_name: &str, method_name: &str) -> bool {
    match type_name {
        "str" => has_str_method(method_name),
        "int" | "bool" => has_int_method(method_name),
        "float" => has_float_method(method_name),
        "list" => has_list_method(method_name),
        "dict" => has_dict_method(method_name),
        "set" => has_set_method(method_name),
        "tuple" => has_tuple_method(method_name),
        "bytes" => has_bytes_method(method_name),
        "frozenset" => has_frozenset_method(method_name),
        _ => false,
    }
}

/// Known methods on `str`.
fn has_str_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "capitalize"
            | "casefold"
            | "center"
            | "count"
            | "encode"
            | "endswith"
            | "expandtabs"
            | "find"
            | "format"
            | "format_map"
            | "index"
            | "isalnum"
            | "isalpha"
            | "isascii"
            | "isdecimal"
            | "isdigit"
            | "isidentifier"
            | "islower"
            | "isnumeric"
            | "isprintable"
            | "isspace"
            | "istitle"
            | "isupper"
            | "join"
            | "ljust"
            | "lower"
            | "lstrip"
            | "maketrans"
            | "partition"
            | "removeprefix"
            | "removesuffix"
            | "replace"
            | "rfind"
            | "rindex"
            | "rjust"
            | "rpartition"
            | "rsplit"
            | "rstrip"
            | "split"
            | "splitlines"
            | "startswith"
            | "strip"
            | "swapcase"
            | "title"
            | "translate"
            | "upper"
            | "zfill"
            | "__contains__"
            | "__getitem__"
            | "__iter__"
            | "__len__"
            | "__add__"
            | "__mul__"
            | "__eq__"
            | "__hash__"
            | "__repr__"
            | "__str__"
    )
}

/// Known methods on `int` and `bool`.
fn has_int_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "__add__"
            | "__sub__"
            | "__mul__"
            | "__truediv__"
            | "__floordiv__"
            | "__mod__"
            | "__pow__"
            | "__neg__"
            | "__pos__"
            | "__abs__"
            | "__eq__"
            | "__ne__"
            | "__lt__"
            | "__le__"
            | "__gt__"
            | "__ge__"
            | "__hash__"
            | "__repr__"
            | "__str__"
            | "__int__"
            | "__float__"
            | "__bool__"
            | "__index__"
            | "bit_length"
            | "bit_count"
            | "to_bytes"
            | "from_bytes"
            | "conjugate"
            | "real"
            | "imag"
    )
}

/// Known methods on `float`.
fn has_float_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "__add__"
            | "__sub__"
            | "__mul__"
            | "__truediv__"
            | "__floordiv__"
            | "__mod__"
            | "__pow__"
            | "__neg__"
            | "__pos__"
            | "__abs__"
            | "__eq__"
            | "__ne__"
            | "__lt__"
            | "__le__"
            | "__gt__"
            | "__ge__"
            | "__hash__"
            | "__repr__"
            | "__str__"
            | "__int__"
            | "__float__"
            | "__bool__"
            | "is_integer"
            | "hex"
            | "fromhex"
            | "conjugate"
            | "real"
            | "imag"
            | "as_integer_ratio"
    )
}

/// Known methods on `list`.
fn has_list_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "append"
            | "clear"
            | "copy"
            | "count"
            | "extend"
            | "index"
            | "insert"
            | "pop"
            | "remove"
            | "reverse"
            | "sort"
            | "__contains__"
            | "__getitem__"
            | "__setitem__"
            | "__delitem__"
            | "__iter__"
            | "__len__"
            | "__add__"
            | "__mul__"
            | "__eq__"
            | "__repr__"
    )
}

/// Known methods on `dict`.
fn has_dict_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "clear"
            | "copy"
            | "fromkeys"
            | "get"
            | "items"
            | "keys"
            | "pop"
            | "popitem"
            | "setdefault"
            | "update"
            | "values"
            | "__contains__"
            | "__getitem__"
            | "__setitem__"
            | "__delitem__"
            | "__iter__"
            | "__len__"
            | "__eq__"
            | "__repr__"
    )
}

/// Known methods on `set`.
fn has_set_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "add"
            | "clear"
            | "copy"
            | "difference"
            | "difference_update"
            | "discard"
            | "intersection"
            | "intersection_update"
            | "isdisjoint"
            | "issubset"
            | "issuperset"
            | "pop"
            | "remove"
            | "symmetric_difference"
            | "symmetric_difference_update"
            | "union"
            | "update"
            | "__contains__"
            | "__iter__"
            | "__len__"
            | "__eq__"
            | "__repr__"
    )
}

/// Known methods on `tuple`.
fn has_tuple_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "count"
            | "index"
            | "__contains__"
            | "__getitem__"
            | "__iter__"
            | "__len__"
            | "__add__"
            | "__mul__"
            | "__eq__"
            | "__hash__"
            | "__repr__"
    )
}

/// Known methods on `bytes`.
fn has_bytes_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "count"
            | "decode"
            | "endswith"
            | "find"
            | "hex"
            | "index"
            | "join"
            | "replace"
            | "split"
            | "startswith"
            | "strip"
            | "upper"
            | "lower"
            | "__contains__"
            | "__getitem__"
            | "__iter__"
            | "__len__"
            | "__add__"
            | "__mul__"
            | "__eq__"
            | "__hash__"
            | "__repr__"
    )
}

/// Known methods on `frozenset`.
fn has_frozenset_method(method_name: &str) -> bool {
    matches!(
        method_name,
        "copy"
            | "difference"
            | "intersection"
            | "isdisjoint"
            | "issubset"
            | "issuperset"
            | "symmetric_difference"
            | "union"
            | "__contains__"
            | "__iter__"
            | "__len__"
            | "__eq__"
            | "__hash__"
            | "__repr__"
    )
}

/// Collect the required members of a protocol class.
fn collect_protocol_members(
    class: &ClassInfo,
    source: &str,
    _functions: &[FunctionInfo],
) -> Vec<ProtocolMember> {
    let mut members = Vec::new();

    // Collect methods.
    for method_name in &class.method_names {
        // Skip dunder methods that come from Protocol/object.
        if is_protocol_inherited_dunder(method_name) {
            continue;
        }

        // Determine if this is a property.
        let is_property = class.method_decorators.iter().any(|(name, decorators)| {
            name == method_name && decorators.iter().any(|d| d == "property")
        });

        let kind = if is_property {
            MemberKind::Property
        } else {
            MemberKind::Method
        };

        members.push(ProtocolMember {
            name: method_name.clone(),
            kind,
            type_text: None,
        });
    }

    // Collect attributes (non-method annotations).
    for attr in &class.attributes {
        // Skip if already collected as a method/property.
        if class.method_names.contains(&attr.name) {
            continue;
        }

        let type_text = attr
            .annotation_span
            .and_then(|sp| slice_span(source, sp))
            .map(|s| s.trim().to_owned());
        members.push(ProtocolMember {
            name: attr.name.clone(),
            kind: MemberKind::Attribute,
            type_text,
        });
    }

    members
}

/// Dunder methods inherited from Protocol/object that don't count as
/// required protocol members.
fn is_protocol_inherited_dunder(name: &str) -> bool {
    matches!(
        name,
        "__init__" | "__new__" | "__init_subclass__" | "__class_getitem__" | "__subclasshook__"
    )
}

/// Parse `TypedDict` field flags from annotation text.
///
/// Returns `(required, read_only)`.
fn parse_typeddict_field_flags(annotation: Option<&str>, total: bool) -> (bool, bool) {
    let Some(ann) = annotation else {
        return (total, false);
    };

    let read_only = ann.contains("ReadOnly[");

    let required = if ann.starts_with("Required[")
        || ann.starts_with("typing.Required[")
        || ann.starts_with("typing_extensions.Required[")
    {
        true
    } else if ann.starts_with("NotRequired[")
        || ann.starts_with("typing.NotRequired[")
        || ann.starts_with("typing_extensions.NotRequired[")
    {
        false
    } else {
        total
    };

    (required, read_only)
}

/// Strip `ReadOnly[...]`, `Required[...]`, `NotRequired[...]` wrappers
/// from a `TypedDict` field annotation, returning the inner type text.
fn strip_typeddict_wrappers(ann: &str) -> &str {
    const PREFIXES: &[&str] = &[
        "ReadOnly[",
        "typing.ReadOnly[",
        "typing_extensions.ReadOnly[",
        "Required[",
        "typing.Required[",
        "typing_extensions.Required[",
        "NotRequired[",
        "typing.NotRequired[",
        "typing_extensions.NotRequired[",
    ];
    let mut result = ann.trim();
    // Peel one layer at a time (handles nesting like ReadOnly[NotRequired[str]]).
    loop {
        let mut stripped = false;
        for prefix in PREFIXES {
            if let Some(inner) = result.strip_prefix(prefix) {
                if let Some(inner) = inner.strip_suffix(']') {
                    result = inner.trim();
                    stripped = true;
                    break;
                }
            }
        }
        if !stripped {
            break;
        }
    }
    result
}

/// Check if a full `InferredType` is a subtype of another using the context.
///
/// This wraps `SubtypeContext::is_subtype` for `Named` types and falls
/// back to `InferredType::is_assignable_to` for structural types.
#[must_use]
pub fn is_subtype_with_context(
    source: &InferredType,
    target: &InferredType,
    ctx: &SubtypeContext<'_>,
) -> bool {
    match (source, target) {
        // Named-to-Named: use the full subtype context.
        (InferredType::Named(source_name), InferredType::Named(target_name)) => {
            ctx.is_subtype(source_name, target_name)
        }

        // Named source to builtin target: check if Named is a subtype.
        (InferredType::Named(source_name), target_builtin) => {
            // TypedDicts are structurally compatible with dict/Mapping.
            if matches!(target_builtin, InferredType::Dict(_, _)) && ctx.is_typeddict(source_name) {
                return true;
            }
            let target_name = inferred_type_to_name(target_builtin);
            if let Some(target_name) = target_name {
                ctx.is_subtype(source_name, target_name)
            } else {
                source.is_assignable_to(target)
            }
        }

        // Builtin source to Named target: check if builtin is a subtype.
        (source_builtin, InferredType::Named(target_name)) => {
            let source_name = inferred_type_to_name(source_builtin);
            if let Some(source_name) = source_name {
                ctx.is_subtype(source_name, target_name)
            } else {
                source.is_assignable_to(target)
            }
        }

        // Union source: all variants must be subtypes.
        (InferredType::Union(types), target) => types
            .iter()
            .all(|t| is_subtype_with_context(t, target, ctx)),

        // Union target: source must be subtype of at least one variant.
        (source, InferredType::Union(types)) => types
            .iter()
            .any(|t| is_subtype_with_context(source, t, ctx)),

        // Optional handling.
        (InferredType::Optional(inner), target) => {
            is_subtype_with_context(inner, target, ctx)
                && is_subtype_with_context(&InferredType::None_, target, ctx)
        }
        (source, InferredType::Optional(inner)) => {
            is_subtype_with_context(source, inner, ctx) || matches!(source, InferredType::None_)
        }

        // Container types with context-aware element checking.
        (InferredType::List(source_elem), InferredType::List(target_elem))
        | (InferredType::Set(source_elem), InferredType::Set(target_elem)) => {
            is_subtype_with_context(source_elem, target_elem, ctx)
        }
        (InferredType::Dict(sk, sv), InferredType::Dict(tk, tv)) => {
            is_subtype_with_context(sk, tk, ctx) && is_subtype_with_context(sv, tv, ctx)
        }
        (InferredType::Tuple(source_elems), InferredType::Tuple(target_elems)) => {
            // Variable-length tuple: tuple[T, ...] is Tuple([T, Named("...")])
            if let Some(target_elem) = crate::types::var_length_tuple_element(target_elems) {
                return source_elems
                    .iter()
                    .all(|s| is_subtype_with_context(s, target_elem, ctx));
            }
            if let Some(source_elem) = crate::types::var_length_tuple_element(source_elems) {
                if matches!(source_elem, InferredType::Any) {
                    return true;
                }
                // Variable-length source cannot satisfy fixed-length target.
                return false;
            }
            // Different lengths with Named elements (unpacked syntax) — assume compatible.
            if source_elems.len() != target_elems.len() {
                return source_elems
                    .iter()
                    .chain(target_elems.iter())
                    .any(|e| matches!(e, InferredType::Named(_)));
            }
            source_elems
                .iter()
                .zip(target_elems.iter())
                .all(|(s, t)| is_subtype_with_context(s, t, ctx))
        }

        // Callable subtyping with context (contravariant params, covariant return).
        (InferredType::Callable(source_info), InferredType::Callable(target_info)) => {
            // Return type: covariant.
            if !is_subtype_with_context(&source_info.return_type, &target_info.return_type, ctx) {
                return false;
            }

            // Ellipsis parameters.
            if source_info.param_types.is_empty() || target_info.param_types.is_empty() {
                return true;
            }

            // Parameter count.
            if source_info.param_types.len() != target_info.param_types.len() {
                return false;
            }

            // Parameters: contravariant.
            source_info
                .param_types
                .iter()
                .zip(target_info.param_types.iter())
                .all(|(sp, tp)| is_subtype_with_context(tp, sp, ctx))
        }

        // Fall back to the existing assignability check.
        _ => source.is_assignable_to(target),
    }
}

/// Map an `InferredType` to its Python type name for MRO lookup.
fn inferred_type_to_name(ty: &InferredType) -> Option<&'static str> {
    match ty {
        InferredType::Int => Some("int"),
        InferredType::Str => Some("str"),
        InferredType::Float => Some("float"),
        InferredType::Bool => Some("bool"),
        InferredType::Bytes => Some("bytes"),
        InferredType::None_ => Some("NoneType"),
        InferredType::List(_) => Some("list"),
        InferredType::Dict(_, _) => Some("dict"),
        InferredType::Set(_) => Some("set"),
        InferredType::Tuple(_) => Some("tuple"),
        _ => None,
    }
}
