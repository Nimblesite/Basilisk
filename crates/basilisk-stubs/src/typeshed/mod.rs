//! Implements [STUBRES-OVERVIEW], [TYPESHEDRT-OVERVIEW], and
//! [STUBRES-TYPESHED-ACQUIRE]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-OVERVIEW
//!
//! Runtime `python/typeshed` acquisition core.
//!
//! Basilisk **never clones** ([STUBRES-TYPESHED-ACQUIRE]). It downloads a commit
//! archive over HTTPS, streams it through four activation gates (Safety, Shape,
//! License, Content), caches the accepted bytes as an immutable ZIP, and reads
//! `.pyi` through an archive VFS.
//!
//! This module tree is deliberately transport-agnostic. The pure security and
//! verification logic — Git-tree reconstruction ([`gittree`]), the activation
//! gates, the in-memory [`archive::Archive`] model, and the composable
//! source-status warnings ([`warning`]) — is defined over an in-memory archive
//! so it is fully unit-testable with no network. The HTTP transport and on-disk
//! cache are thin adapters over the same model, added at a clean seam.
//!
//! **Security boundary** ([STUBRES-TYPESHED-ACQUIRE]): a reported SHA alone
//! proves nothing about the bytes the checker reads. Trusted GitHub metadata
//! binds a commit to its tree; the [`gittree`] reconstruction binds the analysed
//! bytes to that tree. Verification proves *integrity* (bytes match the SHA),
//! never *authenticity* (that the SHA is an official typeshed release) — no
//! release signature is validated, so official provenance ultimately trusts
//! GitHub/TLS. Custom and verification-disabled sources are never labelled
//! official.

pub mod archive;
pub mod bundle;
pub mod cache;
pub mod codec;
pub mod gate;
pub mod gittree;
pub mod manager;
pub mod runtime;
pub mod selector;
pub mod snapshot;
pub mod source;
pub mod transport;
pub mod versions;
pub mod warning;
