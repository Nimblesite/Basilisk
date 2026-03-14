//! Source-level helpers: binary discovery, line/col computation.

/// Path to the pre-built basilisk binary.
///
/// Derives the target directory from the test executable's own location,
/// which works regardless of whether `cargo test` or `cargo llvm-cov`
/// (which uses a different `--target-dir`) invoked us.
#[must_use]
pub fn basilisk_binary() -> String {
    // The test binary lives under <target-dir>/debug/deps/...
    // We want <target-dir>/debug/basilisk
    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent().and_then(|deps| deps.parent()) {
            let candidate = debug_dir.join("basilisk");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    // Fallback to the original hardcoded path.
    format!("{}/../../target/debug/basilisk", env!("CARGO_MANIFEST_DIR"))
}

/// Convert a byte offset in `source` into a 1-based (line, col) pair.
#[must_use]
pub fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let clamped = (offset as usize).min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}
