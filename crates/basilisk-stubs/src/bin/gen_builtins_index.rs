//! Regenerates `data/typeshed/builtins_index.bin` — the precomputed no-target
//! `builtins.pyi` class index ([STUBRES-TYPESHED-BUILTINS-INDEX]).
//!
//! Run after every typeshed bundle refresh:
//!
//! ```sh
//! cargo run -p basilisk-stubs --bin gen_builtins_index
//! ```
//!
//! CI's drift gate (`embedded_index_matches_regenerated_bytes`) fails until
//! the regenerated artifact is committed alongside the new bundle.

use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let bytes = basilisk_stubs::typeshed::builtins_index::regenerate()?;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("data/typeshed/builtins_index.bin");
    std::fs::write(&path, bytes)?;
    Ok(())
}
