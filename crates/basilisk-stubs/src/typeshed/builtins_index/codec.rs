//! Implements [STUBRES-TYPESHED-BUILTINS-INDEX] artifact codec. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BUILTINS-INDEX
//!
//! The on-disk shape of `data/typeshed/builtins_index.bin`.
//!
//! The artifact holds every distinct `builtins.pyi` class map the bundled
//! snapshot can produce — the no-target intersection plus one per target
//! `(3, minor)` interval — over a **shared pool of encoded classes**. Nearly
//! every builtin class is identical across target versions, so pooling stores
//! one copy of each distinct class rather than one copy per variant: the
//! artifact carries six variants for barely more than one variant's bytes.
//!
//! Every read is bounds-checked; corrupt input yields
//! [`BuiltinsIndexError::Malformed`], never a panic, and never a
//! length-prefixed pre-allocation an attacker could size.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::BuiltinsIndexError;
use crate::types::{
    StubClass, StubFunction, StubParam, StubParamKind, StubSpan, StubTarget, StubTargetPlatform,
    StubVariable,
};

/// Artifact magic: name + format version. Bump on any codec change.
pub(super) const MAGIC: &[u8; 8] = b"BSKBIX2\0";

/// The platform dimension of the artifact's key.
///
/// A stub's shape depends on the target platform only through `sys.platform`
/// comparisons, so every platform value that is not one of the literals the
/// stub names is indistinguishable from every other — one [`Self::Other`]
/// class covers all of them ([STUBRES-TYPESHED-BUILTINS-INDEX]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PlatformKey {
    /// No platform evidence: a declaration must hold on every branch.
    All,
    /// One of the exact platform literals the stub compares against.
    Literal(String),
    /// Any platform value the stub never names.
    Other,
}

impl PlatformKey {
    /// The key a concrete target selects, given the stub's own guard literals.
    pub(super) fn of(platform: &StubTargetPlatform, literals: &BTreeSet<String>) -> Self {
        match platform {
            StubTargetPlatform::All => Self::All,
            StubTargetPlatform::Concrete(name) if literals.contains(name) => {
                Self::Literal(name.clone())
            }
            StubTargetPlatform::Concrete(_) => Self::Other,
        }
    }

    fn tag(&self) -> u8 {
        match self {
            Self::All => 0,
            Self::Literal(_) => 1,
            Self::Other => 2,
        }
    }
}

/// One `builtins` class map per (platform class, target-version interval),
/// before encoding.
///
/// Each group's `intervals` is ordered by `min_minor`, starts at `0`, and each
/// entry owns every minor version from its own `min_minor` up to the next
/// entry's — the last entry runs to infinity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Artifact {
    /// The no-target intersection map.
    pub(super) default_classes: HashMap<String, StubClass>,
    /// Per platform class, the `(min_minor, classes)` intervals over `(3, _)`.
    pub(super) groups: Vec<(PlatformKey, Vec<(u8, HashMap<String, StubClass>)>)>,
}

/// A decoded artifact: pooled class blobs plus the variant index that selects
/// them. Class bodies stay encoded until a variant asks for them, so choosing
/// a target decodes exactly one map's worth of classes.
pub(super) struct DecodedArtifact<'bytes> {
    /// Every distinct encoded class, in the artifact's pool order.
    pool: Vec<&'bytes [u8]>,
    /// Pool indices per variant; variant `0` is always the no-target map.
    variants: Vec<Vec<u32>>,
    /// Per platform class, `(min_minor, variant index)` ascending from `0`.
    groups: Vec<(PlatformKey, Vec<(u8, u32)>)>,
}

impl DecodedArtifact<'_> {
    /// The variant a target selects, or `None` when the artifact does not
    /// cover it (a major version other than 3) and the caller must parse live.
    pub(super) fn variant_for(&self, target: Option<&StubTarget>) -> Option<usize> {
        let Some(target) = target else {
            return Some(0);
        };
        let (major, minor) = target.python_version;
        if major != 3 {
            return None;
        }
        let minor = u8::try_from(minor).unwrap_or(u8::MAX);
        let wanted = PlatformKey::of(&target.platform, &self.platform_literals());
        self.groups
            .iter()
            .find(|(key, _)| *key == wanted)?
            .1
            .iter()
            .rev()
            .find(|(start, _)| *start <= minor)
            .map(|(_, variant)| *variant as usize)
    }

    /// The platform literals the artifact was generated against.
    fn platform_literals(&self) -> BTreeSet<String> {
        self.groups
            .iter()
            .filter_map(|(key, _)| match key {
                PlatformKey::Literal(name) => Some(name.clone()),
                PlatformKey::All | PlatformKey::Other => None,
            })
            .collect()
    }

    /// How many variants the artifact carries (the no-target map plus one per
    /// target-version interval).
    #[cfg(test)]
    pub(super) fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// The total pooled byte size of one variant's classes — what that variant
    /// would cost on its own, without sharing.
    #[cfg(test)]
    pub(super) fn pooled_bytes_of_variant(
        &self,
        variant: usize,
    ) -> Result<usize, BuiltinsIndexError> {
        self.variants
            .get(variant)
            .ok_or(BuiltinsIndexError::Malformed(0))?
            .iter()
            .map(|index| {
                self.pool
                    .get(*index as usize)
                    .map(|blob| blob.len())
                    .ok_or(BuiltinsIndexError::Malformed(0))
            })
            .sum()
    }

    /// Decode one variant's classes.
    pub(super) fn classes(
        &self,
        variant: usize,
    ) -> Result<HashMap<String, StubClass>, BuiltinsIndexError> {
        let indices = self
            .variants
            .get(variant)
            .ok_or(BuiltinsIndexError::Malformed(0))?;
        let mut classes = HashMap::with_capacity(indices.len());
        for index in indices {
            let blob = self
                .pool
                .get(*index as usize)
                .ok_or(BuiltinsIndexError::Malformed(0))?;
            let mut cursor = Cursor { bytes: blob, pos: 0 };
            let class = read_class(&mut cursor)?;
            if cursor.pos != blob.len() {
                return Err(BuiltinsIndexError::Malformed(cursor.pos));
            }
            let _ = classes.insert(class.name.clone(), class);
        }
        Ok(classes)
    }
}

// ---------------------------------------------------------------------------
// Encoding — the pool is ordered by encoded bytes and every variant lists its
// classes by name, so the committed artifact is byte-reproducible and the
// drift gate can compare bytes, not semantics.
// ---------------------------------------------------------------------------

pub(super) fn encode(
    artifact: &Artifact,
    bundle_sha_hex: &str,
) -> Result<Vec<u8>, BuiltinsIndexError> {
    let maps: Vec<&HashMap<String, StubClass>> = std::iter::once(&artifact.default_classes)
        .chain(
            artifact
                .groups
                .iter()
                .flat_map(|(_, intervals)| intervals.iter().map(|(_, classes)| classes)),
        )
        .collect();
    let pool = build_pool(&maps)?;
    let slot: BTreeMap<&[u8], u32> = pool
        .iter()
        .enumerate()
        .map(|(index, blob)| (blob.as_slice(), u32::try_from(index).unwrap_or(u32::MAX)))
        .collect();

    let mut out = Vec::with_capacity(1 << 20);
    out.extend_from_slice(MAGIC);
    put_str(&mut out, bundle_sha_hex)?;
    put_len(&mut out, pool.len())?;
    for blob in &pool {
        put_len(&mut out, blob.len())?;
        out.extend_from_slice(blob);
    }
    put_len(&mut out, maps.len())?;
    for map in &maps {
        put_variant(&mut out, map, &slot)?;
    }
    put_groups(&mut out, artifact)
}

/// The platform/version key tables. Variant `0` is the no-target map, so the
/// groups' intervals number their variants from `1` in declaration order.
fn put_groups(out: &mut Vec<u8>, artifact: &Artifact) -> Result<Vec<u8>, BuiltinsIndexError> {
    put_len(out, artifact.groups.len())?;
    let mut variant = 1_usize;
    for (key, intervals) in &artifact.groups {
        out.push(key.tag());
        if let PlatformKey::Literal(name) = key {
            put_str(out, name)?;
        }
        put_len(out, intervals.len())?;
        for (min_minor, _) in intervals {
            out.push(*min_minor);
            put_len(out, variant)?;
            variant = variant.checked_add(1).ok_or(BuiltinsIndexError::LengthOverflow)?;
        }
    }
    Ok(std::mem::take(out))
}

/// Every distinct encoded class across all maps, ordered by its bytes.
fn build_pool(maps: &[&HashMap<String, StubClass>]) -> Result<Vec<Vec<u8>>, BuiltinsIndexError> {
    let mut pool: BTreeSet<Vec<u8>> = BTreeSet::new();
    for map in maps {
        for class in map.values() {
            let mut blob = Vec::new();
            put_class(&mut blob, class)?;
            let _ = pool.insert(blob);
        }
    }
    Ok(pool.into_iter().collect())
}

/// One variant: its classes' pool indices, in class-name order.
fn put_variant(
    out: &mut Vec<u8>,
    map: &HashMap<String, StubClass>,
    slot: &BTreeMap<&[u8], u32>,
) -> Result<(), BuiltinsIndexError> {
    let mut sorted: Vec<(&String, &StubClass)> = map.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    put_len(out, sorted.len())?;
    for (_, class) in sorted {
        let mut blob = Vec::new();
        put_class(&mut blob, class)?;
        let index = slot
            .get(blob.as_slice())
            .copied()
            .ok_or(BuiltinsIndexError::LengthOverflow)?;
        out.extend_from_slice(&index.to_le_bytes());
    }
    Ok(())
}

fn put_len(out: &mut Vec<u8>, len: usize) -> Result<(), BuiltinsIndexError> {
    let value = u32::try_from(len).map_err(|_overflow| BuiltinsIndexError::LengthOverflow)?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_str(out: &mut Vec<u8>, text: &str) -> Result<(), BuiltinsIndexError> {
    put_len(out, text.len())?;
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

fn put_opt_str(out: &mut Vec<u8>, text: Option<&str>) -> Result<(), BuiltinsIndexError> {
    match text {
        None => {
            out.push(0);
            Ok(())
        }
        Some(text) => {
            out.push(1);
            put_str(out, text)
        }
    }
}

pub(super) fn put_class(out: &mut Vec<u8>, class: &StubClass) -> Result<(), BuiltinsIndexError> {
    put_str(out, &class.name)?;
    put_len(out, class.bases.len())?;
    for base in &class.bases {
        put_str(out, base)?;
    }
    put_opt_str(out, class.metaclass.as_deref())?;
    put_len(out, class.methods.len())?;
    for method in &class.methods {
        put_function(out, method)?;
    }
    put_len(out, class.attributes.len())?;
    for attribute in &class.attributes {
        put_variable(out, attribute)?;
    }
    Ok(())
}

fn put_function(out: &mut Vec<u8>, function: &StubFunction) -> Result<(), BuiltinsIndexError> {
    put_str(out, &function.name)?;
    match &function.receiver {
        None => out.push(0),
        Some(receiver) => {
            out.push(1);
            put_param(out, receiver)?;
        }
    }
    put_len(out, function.params.len())?;
    for param in &function.params {
        put_param(out, param)?;
    }
    put_opt_str(out, function.return_type.as_deref())?;
    out.push(u8::from(function.is_overload));
    out.push(u8::from(function.is_async));
    put_len(out, function.decorators.len())?;
    for decorator in &function.decorators {
        put_str(out, decorator)?;
    }
    put_opt_str(out, function.class_name.as_deref())?;
    out.extend_from_slice(&function.source_span.start.to_le_bytes());
    out.extend_from_slice(&function.source_span.end.to_le_bytes());
    Ok(())
}

fn put_param(out: &mut Vec<u8>, param: &StubParam) -> Result<(), BuiltinsIndexError> {
    put_str(out, &param.name)?;
    put_opt_str(out, param.annotation.as_deref())?;
    out.push(u8::from(param.has_default));
    out.push(param_kind_tag(param.kind));
    Ok(())
}

fn put_variable(out: &mut Vec<u8>, variable: &StubVariable) -> Result<(), BuiltinsIndexError> {
    put_str(out, &variable.name)?;
    put_opt_str(out, variable.annotation.as_deref())
}

const fn param_kind_tag(kind: StubParamKind) -> u8 {
    match kind {
        StubParamKind::Regular => 0,
        StubParamKind::Vararg => 1,
        StubParamKind::Kwarg => 2,
        StubParamKind::KeywordOnly => 3,
        StubParamKind::PositionalOnly => 4,
    }
}

const fn param_kind_from(tag: u8) -> Option<StubParamKind> {
    match tag {
        0 => Some(StubParamKind::Regular),
        1 => Some(StubParamKind::Vararg),
        2 => Some(StubParamKind::Kwarg),
        3 => Some(StubParamKind::KeywordOnly),
        4 => Some(StubParamKind::PositionalOnly),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Read the artifact header: the bundle SHA it is bound to, the class pool, and
/// the variant/interval tables. Class bodies are left encoded.
pub(super) fn decode(bytes: &[u8]) -> Result<(String, DecodedArtifact<'_>), BuiltinsIndexError> {
    let mut cursor = Cursor { bytes, pos: 0 };
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(BuiltinsIndexError::Malformed(0));
    }
    let sha = cursor.str()?;
    let pool = read_pool(&mut cursor)?;
    let variants = read_variants(&mut cursor)?;
    let groups = read_groups(&mut cursor)?;
    if cursor.pos != cursor.bytes.len() {
        return Err(BuiltinsIndexError::Malformed(cursor.pos));
    }
    // Variant 0 is the no-target map, and every group must cover minor 0 or a
    // lookup could fall off the front of its interval list.
    let covered = groups
        .iter()
        .all(|(_, intervals)| intervals.first().map(|(start, _)| *start) == Some(0));
    if variants.is_empty() || groups.is_empty() || !covered {
        return Err(BuiltinsIndexError::Malformed(cursor.pos));
    }
    Ok((
        sha,
        DecodedArtifact {
            pool,
            variants,
            groups,
        },
    ))
}

fn read_pool<'bytes>(
    cursor: &mut Cursor<'bytes>,
) -> Result<Vec<&'bytes [u8]>, BuiltinsIndexError> {
    let count = cursor.u32()?;
    let mut pool = Vec::new();
    for _ in 0..count {
        let len = usize::try_from(cursor.u32()?)
            .map_err(|_overflow| BuiltinsIndexError::Malformed(cursor.pos))?;
        pool.push(cursor.take(len)?);
    }
    Ok(pool)
}

fn read_variants(cursor: &mut Cursor<'_>) -> Result<Vec<Vec<u32>>, BuiltinsIndexError> {
    let count = cursor.u32()?;
    let mut variants = Vec::new();
    for _ in 0..count {
        let members = cursor.u32()?;
        let mut indices = Vec::new();
        for _ in 0..members {
            indices.push(cursor.u32()?);
        }
        variants.push(indices);
    }
    Ok(variants)
}

#[expect(
    clippy::type_complexity,
    reason = "the artifact's group table shape; naming it would not clarify a private reader"
)]
fn read_groups(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<(PlatformKey, Vec<(u8, u32)>)>, BuiltinsIndexError> {
    let count = cursor.u32()?;
    let mut groups = Vec::new();
    for _ in 0..count {
        let key = match cursor.u8()? {
            0 => PlatformKey::All,
            1 => PlatformKey::Literal(cursor.str()?),
            2 => PlatformKey::Other,
            _ => return Err(BuiltinsIndexError::Malformed(cursor.pos)),
        };
        groups.push((key, read_intervals(cursor)?));
    }
    Ok(groups)
}

fn read_intervals(cursor: &mut Cursor<'_>) -> Result<Vec<(u8, u32)>, BuiltinsIndexError> {
    let count = cursor.u32()?;
    let mut intervals = Vec::new();
    for _ in 0..count {
        let min_minor = cursor.u8()?;
        intervals.push((min_minor, cursor.u32()?));
    }
    Ok(intervals)
}

struct Cursor<'bytes> {
    bytes: &'bytes [u8],
    pos: usize,
}

impl<'bytes> Cursor<'bytes> {
    fn take(&mut self, len: usize) -> Result<&'bytes [u8], BuiltinsIndexError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(BuiltinsIndexError::Malformed(self.pos))?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(BuiltinsIndexError::Malformed(self.pos))?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, BuiltinsIndexError> {
        let position = self.pos;
        self.take(1)?
            .first()
            .copied()
            .ok_or(BuiltinsIndexError::Malformed(position))
    }

    fn u32(&mut self) -> Result<u32, BuiltinsIndexError> {
        let raw = self.take(4)?;
        let array: [u8; 4] = raw
            .try_into()
            .map_err(|_size| BuiltinsIndexError::Malformed(self.pos))?;
        Ok(u32::from_le_bytes(array))
    }

    fn str(&mut self) -> Result<String, BuiltinsIndexError> {
        let len = usize::try_from(self.u32()?)
            .map_err(|_overflow| BuiltinsIndexError::Malformed(self.pos))?;
        let start = self.pos;
        std::str::from_utf8(self.take(len)?)
            .map(str::to_owned)
            .map_err(|_utf8| BuiltinsIndexError::Malformed(start))
    }

    fn opt_str(&mut self) -> Result<Option<String>, BuiltinsIndexError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.str().map(Some),
            _ => Err(BuiltinsIndexError::Malformed(self.pos)),
        }
    }

    fn bool(&mut self) -> Result<bool, BuiltinsIndexError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(BuiltinsIndexError::Malformed(self.pos)),
        }
    }
}

fn read_class(cursor: &mut Cursor<'_>) -> Result<StubClass, BuiltinsIndexError> {
    let name = cursor.str()?;
    let base_count = cursor.u32()?;
    let mut bases = Vec::new();
    for _ in 0..base_count {
        bases.push(cursor.str()?);
    }
    let metaclass = cursor.opt_str()?;
    let method_count = cursor.u32()?;
    let mut methods = Vec::new();
    for _ in 0..method_count {
        methods.push(read_function(cursor)?);
    }
    let attribute_count = cursor.u32()?;
    let mut attributes = Vec::new();
    for _ in 0..attribute_count {
        attributes.push(read_variable(cursor)?);
    }
    Ok(StubClass {
        name,
        bases,
        metaclass,
        methods,
        attributes,
    })
}

fn read_function(cursor: &mut Cursor<'_>) -> Result<StubFunction, BuiltinsIndexError> {
    let name = cursor.str()?;
    let receiver = match cursor.u8()? {
        0 => None,
        1 => Some(read_param(cursor)?),
        _ => return Err(BuiltinsIndexError::Malformed(cursor.pos)),
    };
    let param_count = cursor.u32()?;
    let mut params = Vec::new();
    for _ in 0..param_count {
        params.push(read_param(cursor)?);
    }
    let return_type = cursor.opt_str()?;
    let is_overload = cursor.bool()?;
    let is_async = cursor.bool()?;
    let decorator_count = cursor.u32()?;
    let mut decorators = Vec::new();
    for _ in 0..decorator_count {
        decorators.push(cursor.str()?);
    }
    let class_name = cursor.opt_str()?;
    let source_span = StubSpan {
        start: cursor.u32()?,
        end: cursor.u32()?,
    };
    Ok(StubFunction {
        name,
        receiver,
        params,
        return_type,
        is_overload,
        is_async,
        decorators,
        class_name,
        source_span,
    })
}

fn read_param(cursor: &mut Cursor<'_>) -> Result<StubParam, BuiltinsIndexError> {
    let name = cursor.str()?;
    let annotation = cursor.opt_str()?;
    let has_default = cursor.bool()?;
    let tag = cursor.u8()?;
    let kind = param_kind_from(tag).ok_or(BuiltinsIndexError::Malformed(cursor.pos))?;
    Ok(StubParam {
        name,
        annotation,
        has_default,
        kind,
    })
}

fn read_variable(cursor: &mut Cursor<'_>) -> Result<StubVariable, BuiltinsIndexError> {
    let name = cursor.str()?;
    let annotation = cursor.opt_str()?;
    Ok(StubVariable { name, annotation })
}
