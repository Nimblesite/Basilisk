//! Basilisk compiled to WebAssembly — check Python in a browser, with no
//! server, no filesystem, and no network.
//!
//! Implements [WASM]. See docs/specs/WASM-SPEC.md
//!
//! Pure logic lives in [`engine`] (testable on the host target). The JS
//! boundary is [`bindings`], compiled only for `wasm32`, so a native
//! `cargo test` exercises everything below it without a wasm runtime
//! ([WASM-BUILD]).

pub mod engine;
pub mod options;
pub mod report;

#[cfg(target_arch = "wasm32")]
mod bindings;

pub use engine::check_source;
pub use options::CheckOptions;
pub use report::{Report, WasmDiagnostic};

/// Check `source` and return the [`Report`] as a JSON string.
///
/// This is the host-side twin of the browser entry point: [`bindings`] is a
/// two-line wrapper over it, so what the tests exercise is what the browser
/// runs ([WASM-API]).
///
/// `options_json` accepts `{}`. Malformed options produce an error report
/// rather than a panic — the JS boundary has no exception contract worth
/// relying on, and a playground should show the reader what it disliked.
#[must_use]
pub fn check_json(source: &str, options_json: &str) -> String {
    let report = match serde_json::from_str::<CheckOptions>(options_json) {
        Ok(options) => check_source(source, &options),
        Err(error) => Report::from_failure(
            options::DEFAULT_PATH,
            &format!("basilisk-wasm: could not parse the options argument: {error}"),
        ),
    };

    serde_json::to_string(&report).unwrap_or_else(|error| {
        // Serializing a `Report` of owned `String`s and `usize`s cannot fail in
        // practice, but the contract is a JSON string, so the fallback is still
        // valid JSON carrying the reason rather than an empty body.
        format!(
            r#"{{"diagnostics":[{{"code":null,"severity":"error","message":"basilisk-wasm: could not serialize the report: {}","path":"{}","line":1,"col":1,"end_line":1,"end_col":1}}]}}"#,
            error.to_string().escape_default(),
            options::DEFAULT_PATH,
        )
    })
}
