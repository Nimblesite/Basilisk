//! Implements the Stage 1 measurement items of
//! [NARROWPLAN-CHECKLIST](../../../docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md):
//! memory on a 1M+ LOC corpus and p50/p99 keystroke re-check latency over the
//! definition-level Salsa queries ([TYPEINF-TARGET-INCREMENTAL],
//! `crates/basilisk-checker/src/incremental_defs.rs`).
//!
//! Usage (generate the corpus first):
//! ```sh
//! python3 scripts/gen_incremental_corpus.py /tmp/incremental-corpus
//! cargo run --release -p basilisk-checker --example incremental_measure -- \
//!     /tmp/incremental-corpus 200
//! ```
//!
//! Methodology (self-measured, reproducible): cold pass runs `definitions` +
//! `definition_type` + `expression_types` + `module_interface` over every
//! file; the keystroke loop then applies single-character body edits to
//! deterministically-chosen files (seeded LCG) and re-runs the same queries
//! for the edited file only, timing each round trip. Memory is the process
//! RSS reported by `ps` after the cold pass and at the end.

use std::io::Write as _;
use std::time::Instant;

use basilisk_checker::incremental_defs::{
    definition_type, definitions, expression_types, module_interface,
};
use basilisk_db::{BasiliskDatabase, Db, SourceFile};
use salsa::Setter as _;

/// Deterministic linear congruential generator (no external RNG dependency).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 16
    }
}

/// Run every definition-level query for one file, returning the definition count.
fn check_file(db: &dyn Db, file: SourceFile) -> usize {
    let defs = definitions(db, file);
    for def in defs {
        let _ = definition_type(db, *def);
        let _ = expression_types(db, *def);
    }
    let _ = module_interface(db, file);
    defs.len()
}

/// Current process RSS in kilobytes, via `ps` (portable across macOS/Linux).
fn rss_kb() -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// The `pct`-th percentile (nearest-rank) of an ascending-sorted sample.
fn percentile(sorted_micros: &[u128], pct: usize) -> u128 {
    let rank = (sorted_micros.len() * pct).div_ceil(100);
    sorted_micros
        .get(rank.saturating_sub(1))
        .or_else(|| sorted_micros.last())
        .copied()
        .unwrap_or(0)
}

/// Format microseconds as fractional milliseconds without float casts.
fn format_ms(micros: u128) -> String {
    format!("{}.{:03}", micros / 1000, micros % 1000)
}

/// Format kilobytes as fractional megabytes without float casts.
fn format_mb(kb: u64) -> String {
    format!("{}.{}", kb / 1024, (kb % 1024) * 10 / 1024)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let corpus_dir = std::path::PathBuf::from(
        args.next()
            .ok_or("usage: incremental_measure CORPUS_DIR [EDITS]")?,
    );
    let edits: usize = args.next().map_or(Ok(200), |raw| raw.parse())?;

    let mut db = BasiliskDatabase::default();
    let mut files: Vec<(SourceFile, String)> = Vec::new();
    let mut total_loc = 0_usize;
    for entry in std::fs::read_dir(&corpus_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "py") {
            let text = std::fs::read_to_string(&path)?;
            total_loc += text.lines().count();
            let file = SourceFile::new(&db, path.display().to_string(), text.clone());
            files.push((file, text));
        }
    }
    files.sort_by_key(|entry| entry.1.len());
    println!("corpus: {} files, {} LOC", files.len(), total_loc);

    // Cold pass.
    let cold_start = Instant::now();
    let mut def_count = 0_usize;
    for (file, _) in &files {
        def_count += check_file(&db, *file);
    }
    let cold = cold_start.elapsed();
    let rss_after_cold = rss_kb().unwrap_or(0);
    println!(
        "cold pass: {def_count} definitions in {:.2}s, RSS {} MB",
        cold.as_secs_f64(),
        format_mb(rss_after_cold)
    );

    // Keystroke loop: append a digit inside a function body line.
    let mut rng = Lcg(0x5eed_2026_0718);
    let mut latencies: Vec<u128> = Vec::with_capacity(edits);
    for round in 0..edits {
        let index = usize::try_from(rng.next()).unwrap_or(0) % files.len().max(1);
        let Some((file, original)) = files.get(index) else {
            continue;
        };
        // Simulate a keystroke: mutate one arithmetic constant in the text.
        let edited = original.replacen("offset * factor", "offset * factor + 1", 1);
        let next_text = if round % 2 == 0 {
            edited
        } else {
            original.clone()
        };

        let start = Instant::now();
        let _ = file.set_text(&mut db).to(next_text);
        let _ = check_file(&db, *file);
        latencies.push(start.elapsed().as_micros());
    }
    latencies.sort_unstable();

    let p50 = percentile(&latencies, 50);
    let p99 = percentile(&latencies, 99);
    let rss_final = rss_kb().unwrap_or(0);
    println!(
        "keystroke re-check over {edits} edits: p50 {} ms, p99 {} ms",
        format_ms(p50),
        format_ms(p99)
    );
    println!("final RSS {} MB", format_mb(rss_final));

    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "RESULT files={} loc={} cold_s={:.2} p50_ms={} p99_ms={} rss_mb={}",
        files.len(),
        total_loc,
        cold.as_secs_f64(),
        format_ms(p50),
        format_ms(p99),
        format_mb(rss_final)
    )?;
    Ok(())
}
