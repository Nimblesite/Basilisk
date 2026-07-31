//! Implements [STUBRES-TYPESHED-BUILTINS-INDEX]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BUILTINS-INDEX
//!
//! Precomputed no-target `builtins.pyi` class index for the bundled snapshot.
//!
//! Parsing `builtins.pyi` and intersecting its guarded branches with no
//! version evidence is the single largest fixed cost on a cold `check`. That
//! extraction is a pure function of the bundled ZIP, so it is computed once at
//! development time (`cargo run -p basilisk-stubs --bin gen_builtins_index`),
//! committed as `data/typeshed/builtins_index.bin`, and embedded here.
//!
//! Safety model: the artifact header carries the manifest bundle SHA-256. A
//! stale, missing, or corrupt artifact makes [`bundled_builtins_classes`]
//! return `None`, and the caller falls back to live extraction — slower,
//! never wrong. The drift gate (`embedded_index_matches_regenerated_bytes`)
//! regenerates the bytes from the real parser in CI, so a bundle refresh that
//! forgets to regenerate the index cannot land.

use std::collections::HashMap;

use super::bundle::{self, BundleError};
use crate::types::{StubClass, StubFunction, StubParam, StubParamKind, StubSpan, StubVariable};

/// The committed precomputed index (see module docs for regeneration).
static EMBEDDED_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/typeshed/builtins_index.bin"
));

/// Artifact magic: name + format version. Bump on any codec change.
const MAGIC: &[u8; 8] = b"BSKBIX1\0";

/// A failure regenerating, encoding, or decoding the precomputed index.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuiltinsIndexError {
    /// The bundled snapshot could not be built.
    #[error("builtins index: bundled snapshot: {0}")]
    Bundle(#[from] BundleError),
    /// The bundled snapshot has no `builtins` stub.
    #[error("builtins index: bundled snapshot has no `builtins` stub")]
    MissingBuiltins,
    /// `builtins.pyi` failed to parse during regeneration.
    #[error("builtins index: parsing builtins.pyi: {0}")]
    Parse(String),
    /// A collection is too large for the u32 length prefix.
    #[error("builtins index: a collection exceeds the u32 length prefix")]
    LengthOverflow,
    /// The artifact bytes are truncated or malformed.
    #[error("builtins index: truncated or malformed artifact at byte {0}")]
    Malformed(usize),
}

/// The precomputed no-target `builtins` class map, when the embedded artifact
/// is present and bound to the current bundle. `None` means "extract live" —
/// callers must treat the two paths as interchangeable.
#[must_use]
pub fn bundled_builtins_classes() -> Option<HashMap<String, StubClass>> {
    let expected = match bundle::manifest_bundle_sha() {
        Ok(sha) => sha,
        Err(error) => {
            tracing::warn!(%error, "builtins index: manifest unavailable; using live extraction");
            return None;
        }
    };
    match decode_classes(EMBEDDED_INDEX) {
        Ok((sha, classes)) if sha == expected => Some(classes),
        Ok((sha, _)) => {
            tracing::warn!(
                embedded = %sha,
                manifest = %expected,
                "builtins index: stale artifact; using live extraction"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "builtins index: undecodable artifact; using live extraction");
            None
        }
    }
}

/// Recompute the artifact bytes from the embedded bundle with the REAL
/// parser — the exact extraction [`bundled_builtins_classes`] replaces.
/// Shared by the `gen_builtins_index` generator and the CI drift gate.
///
/// # Errors
///
/// Returns a [`BuiltinsIndexError`] if the bundle cannot be decoded, the
/// `builtins` stub is missing or unparsable, or encoding overflows.
pub fn regenerate() -> Result<Vec<u8>, BuiltinsIndexError> {
    let snapshot = bundle::bundled_snapshot()?;
    let (logical_uri, source_text) = snapshot
        .read_stub("builtins")
        .ok_or(BuiltinsIndexError::MissingBuiltins)?;
    let module = crate::parse_pyi_source(
        source_text,
        std::path::Path::new(&logical_uri),
        "builtins",
        crate::StubSource::Typeshed,
        crate::StubTier::Tier1,
    )
    .map_err(|error| BuiltinsIndexError::Parse(error.to_string()))?;
    encode_classes(&module.classes, &bundle::manifest_bundle_sha()?)
}

// ---------------------------------------------------------------------------
// Encoding — deterministic (classes sorted by name) so the committed artifact
// is byte-reproducible and the drift gate can compare bytes, not semantics.
// ---------------------------------------------------------------------------

fn encode_classes(
    classes: &HashMap<String, StubClass>,
    bundle_sha_hex: &str,
) -> Result<Vec<u8>, BuiltinsIndexError> {
    let mut sorted: Vec<(&String, &StubClass)> = classes.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    let mut out = Vec::with_capacity(1 << 20);
    out.extend_from_slice(MAGIC);
    put_str(&mut out, bundle_sha_hex)?;
    put_len(&mut out, sorted.len())?;
    for (_, class) in sorted {
        put_class(&mut out, class)?;
    }
    Ok(out)
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

fn put_class(out: &mut Vec<u8>, class: &StubClass) -> Result<(), BuiltinsIndexError> {
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
// Decoding — every read is bounds-checked; corrupt input yields `Malformed`,
// never a panic or over-allocation (no length-prefix preallocation).
// ---------------------------------------------------------------------------

fn decode_classes(
    bytes: &[u8],
) -> Result<(String, HashMap<String, StubClass>), BuiltinsIndexError> {
    let mut cursor = Cursor { bytes, pos: 0 };
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(BuiltinsIndexError::Malformed(0));
    }
    let sha = cursor.str()?;
    let count = cursor.u32()?;
    let mut classes = HashMap::new();
    for _ in 0..count {
        let class = read_class(&mut cursor)?;
        let _ = classes.insert(class.name.clone(), class);
    }
    if cursor.pos != cursor.bytes.len() {
        return Err(BuiltinsIndexError::Malformed(cursor.pos));
    }
    Ok((sha, classes))
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

#[cfg(test)]
#[path = "builtins_index/tests.rs"]
mod tests;
