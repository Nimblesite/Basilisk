//! Implements [STUBRES-TYPESHED-BUILTINS-INDEX]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BUILTINS-INDEX
//!
//! Precomputed `builtins.pyi` class index for the bundled snapshot.
//!
//! Parsing `builtins.pyi` and resolving its guarded branches is the single
//! largest fixed cost on a cold `check` — ~3 ms of the run, every run. That
//! extraction is a pure function of the bundled ZIP and the target version, so
//! it is computed once at development time (`cargo run -p basilisk-stubs --bin
//! gen_builtins_index`), committed as `data/typeshed/builtins_index.bin`, and
//! embedded here.
//!
//! **Every** target is covered, not just the no-target intersection: a project
//! that pins `python-version` is the common case, and serving only the
//! unpinned case would leave that project paying the live parse on every
//! invocation. `builtins.pyi`'s class map is a step function of the target
//! version, and of the target platform only through the `sys.platform`
//! literals the stub itself names, so the artifact enumerates one variant per
//! (platform class, minor-version interval) — a provably complete, finite set
//! — over a shared class pool that keeps the repetition out of the bytes
//! ([`codec`]).
//!
//! Safety model: the artifact header carries the manifest bundle SHA-256. A
//! stale, missing, or corrupt artifact makes [`bundled_builtins_classes`]
//! return `None`, and the caller falls back to live extraction — slower,
//! never wrong. The drift gate (`embedded_index_matches_regenerated_bytes`)
//! regenerates the bytes from the real parser in CI, so a bundle refresh that
//! forgets to regenerate the index cannot land.

mod codec;

use std::collections::HashMap;

use codec::{Artifact, PlatformKey};

use super::bundle::{self, BundleError};
use crate::types::{StubClass, StubTarget, StubTargetPlatform};

/// The committed precomputed index (see module docs for regeneration).
static EMBEDDED_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/typeshed/builtins_index.bin"
));

/// Highest `(3, minor)` the generator materialises. Guarded branches in
/// `builtins.pyi` compare against versions the file itself names, so the map
/// is constant above the largest one; generating well past it makes the final
/// interval's open-ended reading a measured fact rather than an assumption.
const MAX_GENERATED_MINOR: u8 = 40;

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
    /// Two platform values that `builtins.pyi` never names produced different
    /// class maps, so "every unnamed platform behaves alike" — the premise
    /// behind the artifact's single `Other` platform class — does not hold.
    #[error(
        "builtins index: unnamed platforms {left} and {right} extract different class maps at \
         3.{minor}; the artifact's single fallback platform class cannot represent both"
    )]
    PlatformFallbackSplit {
        /// First probe platform.
        left: String,
        /// Second probe platform, which disagreed with it.
        right: String,
        /// The target minor version where they diverged.
        minor: u8,
    },
}

/// The precomputed `builtins` class map for `target`, when the embedded
/// artifact is present, bound to the current bundle, and covers that target.
/// `None` means "extract live" — callers must treat the two paths as
/// interchangeable.
///
/// `target`'s platform selects a variant through the stub's OWN guard
/// literals: an exact match on one it names, otherwise the single class
/// covering every platform it does not — a partition regeneration proves is
/// complete ([`BuiltinsIndexError::PlatformFallbackSplit`]).
#[must_use]
pub fn bundled_builtins_classes(target: Option<&StubTarget>) -> Option<HashMap<String, StubClass>> {
    let expected = match bundle::manifest_bundle_sha() {
        Ok(sha) => sha,
        Err(error) => {
            tracing::warn!(%error, "builtins index: manifest unavailable; using live extraction");
            return None;
        }
    };
    let (sha, artifact) = codec::decode(EMBEDDED_INDEX)
        .inspect_err(
            |error| tracing::warn!(%error, "builtins index: undecodable artifact; using live extraction"),
        )
        .ok()?;
    if sha != expected {
        tracing::warn!(
            embedded = %sha,
            manifest = %expected,
            "builtins index: stale artifact; using live extraction"
        );
        return None;
    }
    let variant = artifact.variant_for(target)?;
    artifact
        .classes(variant)
        .inspect_err(
            |error| tracing::warn!(%error, "builtins index: unreadable variant; using live extraction"),
        )
        .ok()
}

/// Recompute the artifact bytes from the embedded bundle with the REAL
/// parser — the exact extraction [`bundled_builtins_classes`] replaces.
/// Shared by the `gen_builtins_index` generator and the CI drift gate.
///
/// # Errors
///
/// Returns a [`BuiltinsIndexError`] if the bundle cannot be decoded, the
/// `builtins` stub is missing or unparsable, encoding overflows, or the stub
/// grew a platform guard the version-keyed artifact cannot express.
pub fn regenerate() -> Result<Vec<u8>, BuiltinsIndexError> {
    let snapshot = bundle::bundled_snapshot()?;
    let (logical_uri, source_text) = snapshot
        .read_stub("builtins")
        .ok_or(BuiltinsIndexError::MissingBuiltins)?;
    let path = std::path::Path::new(&logical_uri);
    let literals = crate::pyi_parser::platform_guard_literals(source_text, path)
        .map_err(|error| BuiltinsIndexError::Parse(error.to_string()))?;
    let mut groups = vec![(
        PlatformKey::All,
        extract_intervals(&logical_uri, source_text, &StubTargetPlatform::All)?,
    )];
    for literal in &literals {
        groups.push((
            PlatformKey::Literal(literal.clone()),
            extract_intervals(
                &logical_uri,
                source_text,
                &StubTargetPlatform::Concrete(literal.clone()),
            )?,
        ));
    }
    groups.push((
        PlatformKey::Other,
        extract_unnamed_platform_intervals(&logical_uri, source_text, &literals)?,
    ));
    let artifact = Artifact {
        default_classes: extract_untargeted(&logical_uri, source_text)?,
        groups,
    };
    codec::encode(&artifact, &bundle::manifest_bundle_sha()?)
}

/// Platforms the stub never names, used to pin down its `Other` class. They
/// are spread across the string ordering so an ordered `sys.platform`
/// comparison — which would split the fallback class in two — cannot pass
/// unnoticed.
const UNNAMED_PLATFORM_PROBES: [&str; 3] = [
    "basilisk-unnamed-aaaa",
    "basilisk-unnamed-mmmm",
    "basilisk-unnamed-zzzz",
];

/// The intervals for every platform the stub does not name, proven to be one
/// class by extracting each probe and requiring them all to agree.
fn extract_unnamed_platform_intervals(
    logical_uri: &str,
    source_text: &str,
    literals: &std::collections::BTreeSet<String>,
) -> Result<Vec<(u8, HashMap<String, StubClass>)>, BuiltinsIndexError> {
    let probes: Vec<&str> = UNNAMED_PLATFORM_PROBES
        .into_iter()
        .filter(|probe| !literals.contains(*probe))
        .collect();
    let mut agreed: Option<(&str, Vec<(u8, HashMap<String, StubClass>)>)> = None;
    for probe in probes {
        let platform = StubTargetPlatform::Concrete((*probe).to_owned());
        let intervals = extract_intervals(logical_uri, source_text, &platform)?;
        match &agreed {
            None => agreed = Some((probe, intervals)),
            Some((first, expected)) if *expected != intervals => {
                return Err(disagreement(first, probe, expected, &intervals))
            }
            Some(_) => {}
        }
    }
    agreed
        .map(|(_, intervals)| intervals)
        .ok_or(BuiltinsIndexError::MissingBuiltins)
}

/// Name the first minor version at which two probes' intervals diverge.
fn disagreement(
    left: &str,
    right: &str,
    expected: &[(u8, HashMap<String, StubClass>)],
    actual: &[(u8, HashMap<String, StubClass>)],
) -> BuiltinsIndexError {
    let minor = expected
        .iter()
        .zip(actual)
        .find(|(one, other)| one != other)
        .map_or(0, |(one, _)| one.0);
    BuiltinsIndexError::PlatformFallbackSplit {
        left: left.to_owned(),
        right: right.to_owned(),
        minor,
    }
}

/// The no-target intersection map.
fn extract_untargeted(
    logical_uri: &str,
    source_text: &str,
) -> Result<HashMap<String, StubClass>, BuiltinsIndexError> {
    crate::parse_pyi_source(
        source_text,
        std::path::Path::new(logical_uri),
        "builtins",
        crate::StubSource::Typeshed,
        crate::StubTier::Tier1,
    )
    .map(|module| module.classes)
    .map_err(|error| BuiltinsIndexError::Parse(error.to_string()))
}

/// One `(min_minor, classes)` entry per distinct map across `(3, minor)` at a
/// fixed platform. Consecutive minors that extract the same map collapse into
/// one interval.
fn extract_intervals(
    logical_uri: &str,
    source_text: &str,
    platform: &StubTargetPlatform,
) -> Result<Vec<(u8, HashMap<String, StubClass>)>, BuiltinsIndexError> {
    let mut intervals: Vec<(u8, HashMap<String, StubClass>)> = Vec::new();
    for minor in 0..=MAX_GENERATED_MINOR {
        let classes = extract_for_target(logical_uri, source_text, minor, platform)?;
        if intervals
            .last()
            .is_none_or(|(_, previous)| *previous != classes)
        {
            intervals.push((minor, classes));
        }
    }
    Ok(intervals)
}

fn extract_for_target(
    logical_uri: &str,
    source_text: &str,
    minor: u8,
    platform: &StubTargetPlatform,
) -> Result<HashMap<String, StubClass>, BuiltinsIndexError> {
    let target = StubTarget {
        python_version: (3, u32::from(minor)),
        platform: platform.clone(),
    };
    crate::pyi_parser::parse_pyi_source_for_target(
        source_text,
        std::path::Path::new(logical_uri),
        "builtins",
        crate::StubSource::Typeshed,
        crate::StubTier::Tier1,
        &target,
    )
    .map(|module| module.classes)
    .map_err(|error| BuiltinsIndexError::Parse(error.to_string()))
}

#[cfg(test)]
#[path = "builtins_index/tests.rs"]
mod tests;
