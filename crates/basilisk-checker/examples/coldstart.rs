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
    let target = basilisk_stubs::types::StubTarget {
        python_version: (3, 13),
        platform: basilisk_stubs::types::StubTargetPlatform::Concrete("darwin".to_owned()),
    };
    let t3 = Instant::now();
    basilisk_checker::imports::prewarm_builtin_classes(&arc, Some(&target));
    println!(
        "prewarm WITH target(3,13)   {:>8.3} ms",
        t3.elapsed().as_secs_f64() * 1e3
    );
    let t2 = Instant::now();
    basilisk_checker::imports::prewarm_builtin_classes(&arc, None);
    let t_prewarm = t2.elapsed();

    println!("bundled_snapshot()          {:>8.3} ms", t_snapshot.as_secs_f64() * 1e3);
    println!("bundled_builtins_classes()  {:>8.3} ms ({} classes)", t_index.as_secs_f64() * 1e3, classes.map_or(0, |c| c.len()));
    println!("prewarm_builtin_classes()   {:>8.3} ms", t_prewarm.as_secs_f64() * 1e3);
    // How many DISTINCT builtins class maps exist across target minor versions?
    {
        let snap = basilisk_stubs::typeshed::bundle::bundled_snapshot().expect("bundle");
        let (uri, text) = snap.read_stub("builtins").expect("builtins");
        println!("builtins.pyi bytes = {}", text.len());
        let t = Instant::now();
        let m = basilisk_stubs::parse_pyi_source(
            text,
            std::path::Path::new(&uri),
            "builtins",
            basilisk_stubs::StubSource::Typeshed,
            basilisk_stubs::StubTier::Tier1,
        )
        .expect("parse");
        println!(
            "live no-target parse        {:>8.3} ms ({} classes)",
            t.elapsed().as_secs_f64() * 1e3,
            m.classes.len()
        );
        for _ in 0..3 {
            let t = Instant::now();
            let parsed = basilisk_parser::parse_source(text.to_owned(), uri.clone());
            println!(
                "  raw ruff parse only       {:>8.3} ms (ok={})",
                t.elapsed().as_secs_f64() * 1e3,
                parsed.is_ok()
            );
        }
        for _ in 0..3 {
            let t = Instant::now();
            let m2 = basilisk_stubs::parse_pyi_source(
                text,
                std::path::Path::new(&uri),
                "builtins",
                basilisk_stubs::StubSource::Typeshed,
                basilisk_stubs::StubTier::Tier1,
            );
            println!(
                "  parse_pyi_source (warm)   {:>8.3} ms (ok={})",
                t.elapsed().as_secs_f64() * 1e3,
                m2.is_ok()
            );
        }
        let mut seen: std::collections::BTreeMap<String, Vec<u32>> = Default::default();
        for minor in 0u32..=25 {
            let target = basilisk_stubs::types::StubTarget {
                python_version: (3, minor),
                platform: basilisk_stubs::types::StubTargetPlatform::All,
            };
            let parsed = basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
                text,
                std::path::Path::new(&uri),
                "builtins",
                basilisk_stubs::StubSource::Typeshed,
                basilisk_stubs::StubTier::Tier1,
                &target,
            )
            .expect("parse");
            let key = format!("{:?}", {
                let mut v: Vec<_> = parsed
                    .classes
                    .iter()
                    .map(|(n, c)| {
                        (
                            n.clone(),
                            c.methods.len(),
                            c.attributes.len(),
                            c.bases.clone(),
                        )
                    })
                    .collect();
                v.sort();
                v
            });
            seen.entry(key).or_default().push(minor);
        }
        println!("distinct builtins maps across 3.0–3.25: {}", seen.len());
        for group in seen.values() {
            println!("   minors {group:?}");
        }
    }
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
