//! Implements [STUBRES-TYPESHED-PIN] commit-object reconstruction.
//!
//! GitHub's commit API reports the attested content (`verification.payload`)
//! and any signature separately; Git stores the signature inside the commit
//! object as a `gpgsig` multi-line header. Reassembling them byte-exactly lets
//! the download prove — before anything is stored — that the object it will
//! save hashes to the requested SHA, and gives the checker the raw object it
//! later re-hashes offline. A reconstruction that does not hash to the
//! requested SHA fails the download; nothing is written.

use basilisk_stubs::typeshed::gittree::{commit_root_tree, git_commit_oid, Oid};

/// A verified raw commit object and the identities it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitObject {
    /// The raw object content (`git cat-file commit` bytes).
    pub raw: Vec<u8>,
    /// The commit SHA the raw content hashes to.
    pub commit: Oid,
    /// The root-tree SHA named by the verified content.
    pub tree: Oid,
}

/// Why reconstruction was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    /// The payload had no header/message boundary to place a signature at.
    #[error("commit payload is malformed")]
    MalformedPayload,
    /// The reconstructed object does not hash to the SHA GitHub reported.
    #[error("reconstructed commit object does not hash to the reported sha")]
    HashMismatch,
    /// The verified object's tree header is missing or malformed.
    #[error("verified commit object has no readable tree header")]
    MalformedTree,
}

/// Reassemble and verify the raw commit object for `expected`.
///
/// The signature (when present) is re-inserted as Git encodes it: a `gpgsig `
/// header whose continuation lines — including the one produced by the
/// signature's own trailing newline — are prefixed with a single space.
///
/// # Errors
///
/// Returns [`CommitError`] when the payload is malformed, the reconstruction
/// does not hash to `expected`, or the verified object names no tree.
pub fn reconstruct(
    payload: &str,
    signature: Option<&str>,
    expected: Oid,
) -> Result<CommitObject, CommitError> {
    let raw = assemble(payload, signature)?;
    let commit = git_commit_oid(&raw);
    if commit != expected {
        return Err(CommitError::HashMismatch);
    }
    let tree = commit_root_tree(&raw).map_err(|_error| CommitError::MalformedTree)?;
    Ok(CommitObject { raw, commit, tree })
}

fn assemble(payload: &str, signature: Option<&str>) -> Result<Vec<u8>, CommitError> {
    let Some(signature) = signature else {
        return Ok(payload.as_bytes().to_vec());
    };
    // Headers end at the first blank line; the signature header goes last,
    // directly before it, exactly where git puts it.
    let boundary = payload.find("\n\n").ok_or(CommitError::MalformedPayload)?;
    let mut raw = String::with_capacity(payload.len() + signature.len() + 16);
    raw.push_str(&payload[..=boundary]);
    raw.push_str("gpgsig ");
    raw.push_str(&signature.replace('\n', "\n "));
    raw.push('\n');
    raw.push_str(&payload[boundary + 1..]);
    Ok(raw.into_bytes())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixtures parse a vendored real API response"
)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// A real `python/typeshed` API response (the bundled commit), captured
    /// verbatim: GPG-signed by GitHub's web-flow key, so it exercises the
    /// signature re-insertion path against ground truth.
    const REAL_COMMIT_JSON: &str = include_str!("../testdata/commit_83c2518.json");

    #[derive(Deserialize)]
    struct Fixture {
        sha: String,
        tree: String,
        payload: String,
        signature: Option<String>,
    }

    fn fixture() -> Fixture {
        serde_json::from_str(REAL_COMMIT_JSON).expect("valid fixture json")
    }

    /// The load-bearing test: a real signed typeshed commit reassembles
    /// byte-exactly and hashes to its published SHA. If GitHub's payload
    /// encoding or our continuation encoding ever drifts, this fails — and so
    /// would every real download, loudly, writing nothing.
    #[test]
    fn a_real_signed_typeshed_commit_reconstructs_to_its_published_sha() {
        let fixture = fixture();
        let expected = Oid::from_hex(&fixture.sha).expect("fixture sha");
        let object = reconstruct(&fixture.payload, fixture.signature.as_deref(), expected)
            .expect("real commit must reconstruct");
        assert_eq!(object.commit, expected);
        assert_eq!(object.tree.to_hex(), fixture.tree);
        // The raw object is exactly what `git cat-file commit` prints: headers,
        // gpgsig continuation (including the trailing `\n ` line), blank, message.
        let text = String::from_utf8(object.raw).expect("utf8 commit object");
        assert!(text.starts_with(&format!("tree {}\n", fixture.tree)));
        assert!(text.contains("\ngpgsig -----BEGIN PGP SIGNATURE-----\n"));
        assert!(text.contains("-----END PGP SIGNATURE-----\n \n\n"));
    }

    #[test]
    fn an_unsigned_commit_is_the_payload_verbatim() {
        let payload = "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nx\n";
        let expected = git_commit_oid(payload.as_bytes());
        let object = reconstruct(payload, None, expected).expect("unsigned commit");
        assert_eq!(object.raw, payload.as_bytes());
        assert_eq!(
            object.tree.to_hex(),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
    }

    /// Any tampering — payload or signature — misses the SHA and is terminal.
    #[test]
    fn a_tampered_payload_or_signature_fails_the_hash() {
        let fixture = fixture();
        let expected = Oid::from_hex(&fixture.sha).expect("fixture sha");
        let tampered_payload = fixture.payload.replace("Refactor", "Sabotage");
        assert_eq!(
            reconstruct(&tampered_payload, fixture.signature.as_deref(), expected).err(),
            Some(CommitError::HashMismatch)
        );
        let tampered_signature = fixture
            .signature
            .as_deref()
            .map(|signature| signature.replace("PGP", "GPG"));
        assert_eq!(
            reconstruct(&fixture.payload, tampered_signature.as_deref(), expected).err(),
            Some(CommitError::HashMismatch)
        );
        // Dropping the signature from a signed commit also misses the SHA.
        assert_eq!(
            reconstruct(&fixture.payload, None, expected).err(),
            Some(CommitError::HashMismatch)
        );
    }

    #[test]
    fn a_signed_payload_without_a_header_boundary_is_malformed() {
        assert_eq!(
            reconstruct("tree only no blank line", Some("sig"), git_commit_oid(b"x")).err(),
            Some(CommitError::MalformedPayload)
        );
    }
}
