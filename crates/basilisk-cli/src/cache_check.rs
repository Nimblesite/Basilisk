//! Implements [CHKCACHE-CLI]. See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-CLI
//!
//! CLI surface of the opt-in persistent result cache. The mechanism itself —
//! fingerprinting, lookup/store, the recorder-wrapped cold check — is the
//! shared core in [`basilisk_checker::result_cache`], so the CLI and the
//! language server ([CHKCACHE-LSP]) provably run one cache, not two. This
//! module only maps the `--cache*` flags onto that core.

pub use basilisk_checker::result_cache::{
    build_context, check_file, CacheOptions, CacheOverride, CacheStats,
};
