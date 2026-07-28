//! Implements [WASM-API]. See docs/specs/WASM-SPEC.md#WASM-API
//!
//! The JS boundary, and nothing else. Every decision lives in
//! [`crate::check_json`], which the host tests exercise directly, so this file
//! stays thin enough that the untested-in-CI surface is a single call.
//!
//! Compiled only for `wasm32` ([WASM-BUILD]).

// `#[wasm_bindgen]` expands to `unsafe` shims and undocumented items for the JS
// ABI. The workspace denies both, and rightly so for hand-written code; this
// module exists purely to host that generated boundary, and contains no
// hand-written unsafe of its own.
#![allow(
    unsafe_code,
    missing_docs,
    reason = "wasm_bindgen generates the unsafe JS ABI shims and undocumented glue items"
)]

use wasm_bindgen::prelude::wasm_bindgen;

/// Check `source` and return a JSON [`crate::Report`].
///
/// `options_json` accepts `{}` — see [WASM-API] for the fields.
#[wasm_bindgen]
#[must_use]
pub fn check(source: &str, options_json: &str) -> String {
    crate::check_json(source, options_json)
}
