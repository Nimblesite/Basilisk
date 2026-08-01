//! TEMPORARY cold-start phase timer (delete before commit).
#![allow(clippy::all, clippy::pedantic, missing_docs, unused_crate_dependencies)]

use std::time::Instant;

use basilisk_stubs::typeshed::codec::{decode_zip_static, DecodeLimits, ZipLayout};

fn main() {
    let t0 = Instant::now();
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot().expect("bundle");
    let t_snapshot = t0.elapsed();

    let t1 = Instant::now();
    let classes = basilisk_stubs::typeshed::builtins_index::bundled_builtins_classes();
    let t_index = t1.elapsed();

    let arc = std::sync::Arc::new(snapshot);
    let t2 = Instant::now();
    basilisk_checker::imports::prewarm_builtin_classes(&arc, None);
    let t_prewarm = t2.elapsed();

    println!("bundled_snapshot()          {:>8.3} ms", t_snapshot.as_secs_f64() * 1e3);
    println!("bundled_builtins_classes()  {:>8.3} ms ({} classes)", t_index.as_secs_f64() * 1e3, classes.map_or(0, |c| c.len()));
    println!("prewarm_builtin_classes()   {:>8.3} ms", t_prewarm.as_secs_f64() * 1e3);
    println!("--- sub-phases of bundled_snapshot (re-run, warm caches) ---");

    // Re-time the decode alone against the same static bytes.
    let bytes: &'static [u8] = Box::leak(std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../basilisk-stubs/data/typeshed/stdlib.zip"
    )).expect("zip").into_boxed_slice());
    for _ in 0..3 {
        let t = Instant::now();
        let archive = decode_zip_static(bytes, ZipLayout::BundledRootless, &DecodeLimits::default()).expect("decode");
        let d = t.elapsed();
        println!("decode_zip_static           {:>8.3} ms ({} entries)", d.as_secs_f64() * 1e3, archive.len());
        let identity = basilisk_stubs::typeshed::source::SourceIdentity::Bundled {
            commit: basilisk_stubs::typeshed::gittree::Oid::from_hex(
                basilisk_stubs::typeshed::bundle::bundled_commit_sha(),
            )
            .expect("sha"),
        };
        let t = Instant::now();
        let vfs =
            basilisk_stubs::typeshed::archive::ArchiveVfs::new(identity.uri_component(), archive);
        let d = t.elapsed();
        println!("ArchiveVfs::new             {:>8.3} ms", d.as_secs_f64() * 1e3);
        let t = Instant::now();
        let built = basilisk_stubs::typeshed::snapshot::Snapshot::build(
            identity,
            basilisk_stubs::typeshed::bundle::bundled_pinned_status(true).expect("status"),
            vfs,
            None,
        );
        let d = t.elapsed();
        println!(
            "Snapshot::build             {:>8.3} ms (ok={})",
            d.as_secs_f64() * 1e3,
            built.is_ok()
        );
    }
}
