//! Implements [STUBRES-OVERVIEW], [TYPESHEDRT-OVERVIEW], and
//! [STUBRES-TYPESHED-OFFLINE]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-OVERVIEW
//!
//! Runtime `python/typeshed` source resolution core.
//!
//! There are exactly two sources, both already on this machine when checking
//! starts ([STUBRES-TYPESHED]): a **pinned commit** (the embedded bundle when
//! the SHA is the bundled one, else that commit's [`store`] entry) or a
//! **custom folder**. Resolution performs no network activity of any kind —
//! structurally: this crate links no HTTP client, so the analysis path cannot
//! reach the network even by mistake ([STUBRES-TYPESHED-OFFLINE]). Downloading
//! lives in the separate `basilisk-typeshed-fetch` crate and runs only on
//! explicit user action ([STUBRES-TYPESHED-DOWNLOAD]).
//!
//! **Security boundary** ([STUBRES-TYPESHED-PIN]): a pin is a verification —
//! the stored raw commit object must hash to the pinned SHA, and the stored
//! tree must re-hash to the root tree that verified commit object names. This
//! proves *integrity* since acquisition (bytes match the SHA), never
//! *authenticity* (that the SHA is an official typeshed commit) — no release
//! signature exists, so official provenance ultimately trusts GitHub/TLS at
//! download time. There is no verification waiver.

pub mod archive;
pub mod builtins_index;
pub mod bundle;
pub mod codec;
pub mod gate;
pub mod gittree;
pub mod manager;
pub mod runtime;
pub mod selector;
pub mod snapshot;
pub mod source;
pub mod store;
pub mod versions;
pub mod warning;
