#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! PEP conformance test harness.
//!
//! Runs every `.py` file from the `python/typing` conformance suite against
//! the Basilisk pipeline and prints a scored report.
//!
//! ## Prerequisites
//!
//! The conformance files must be downloaded first:
//!
//! ```text
//! ./conformance/fetch-conformance.sh
//! cargo test --test conformance_tests -- --nocapture
//! ```
//!
//! ## Annotation format (from the python/typing spec)
//!
//! Each line in a conformance file may carry one of these trailing comments:
//!
//! | Annotation  | Meaning                                               |
//! |-------------|-------------------------------------------------------|
//! | `# E`       | A type error MUST be reported on this line            |
//! | `# E?`      | A type error MAY be reported (optional)               |
//! | `# E[tag]`  | Exactly one line sharing this tag must error          |
//! | `# E[tag+]` | One or more lines sharing this tag may error          |
//!
//! Anything after the annotation (e.g. `# E: some explanation`) is ignored.
//!
//! ## Scoring
//!
//! A file **passes** when every required `# E` line has at least one
//! diagnostic from Basilisk.  Optional `# E?` lines and tag groups are
//! tracked but do not affect pass/fail.  False positives (Basilisk reports
//! errors on unmarked lines) are counted separately for visibility.
//!
//! ## Skip behaviour
//!
//! If the conformance directory does not exist the test prints a clear message
//! and exits with success so that CI on a fresh checkout does not break.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
};

use basilisk_checker::check;
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

// ---------------------------------------------------------------------------
// Annotation parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Annotation {
    /// Error must be reported on this line.
    Required,
    /// Error may optionally be reported.
    Optional,
    /// Tagged group: exactly one line with this tag must error.
    TaggedExact(String),
    /// Tagged group: one or more lines with this tag may error.
    TaggedMulti(String),
}

/// Parse a single source line and return the annotation, if any.
fn parse_annotation(line: &str) -> Option<Annotation> {
    // Skip full-line comments — a `# E` inside a comment is not a real
    // annotation because the line contains no executable code for the
    // checker to flag.
    if line.trim_start().starts_with('#') {
        // Allow lines that are ONLY a `# E` marker (pure annotation lines
        // are used in some conformance files), but skip lines where real
        // code has been commented out with a trailing `# E`.
        let trimmed = line.trim();
        // Pure annotation: `# E`, `# E: explanation`, `# E[tag]`, `# E?`
        let after_hash = trimmed.strip_prefix('#')?.trim_start();
        if !after_hash.starts_with('E') {
            return None;
        }
    }

    // Find the last `# E` marker on the line.
    let marker = line.rfind("# E")?;
    let rest = line[marker + 2..].trim(); // everything after "#"

    if rest.starts_with("E?") {
        return Some(Annotation::Optional);
    }

    if rest.starts_with("E[") {
        let inner = rest.strip_prefix("E[")?;
        // Find closing ] — ignore anything after it (description text)
        if let Some(close) = inner.find(']') {
            let tag = &inner[..close];
            if tag.ends_with('+') {
                return Some(Annotation::TaggedMulti(
                    tag.trim_end_matches('+').to_owned(),
                ));
            }
            return Some(Annotation::TaggedExact(tag.to_owned()));
        }
        // No closing ] at all — malformed, treat as required
        return Some(Annotation::Required);
    }

    // `# E` possibly followed by `: explanation` or nothing
    if rest.starts_with('E') {
        let after = rest["E".len()..].trim_start();
        if after.is_empty() || after.starts_with(':') || after.starts_with(' ') {
            return Some(Annotation::Required);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Line-number helper (byte offset → 1-based line)
// ---------------------------------------------------------------------------

fn byte_offset_to_line(source: &str, offset: u32) -> usize {
    let clamped = (offset as usize).min(source.len());
    source[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

// ---------------------------------------------------------------------------
// Per-file result
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct FileResult {
    /// `# E` lines that Basilisk caught.
    caught: usize,
    /// `# E` lines that Basilisk missed.
    missed: usize,
    /// Lines Basilisk flagged that had no annotation (false positives).
    false_positives: usize,
    /// `# E?` optional lines where Basilisk did fire.
    #[expect(dead_code, reason = "tracked for future reporting")]
    optional_caught: usize,
    /// `# E[tag]` groups satisfied.
    tagged_exact_satisfied: usize,
    /// `# E[tag]` groups missed.
    tagged_exact_missed: usize,
    /// Distinct Basilisk rule codes fired on this file (conformance-relevant only).
    rules_fired: Vec<String>,
}

impl FileResult {
    fn passes(&self) -> bool {
        self.missed == 0
    }
}

// ---------------------------------------------------------------------------
// Annotation collection
// ---------------------------------------------------------------------------

struct Annotations {
    required: HashSet<usize>,
    optional: HashSet<usize>,
    tagged_exact: HashMap<String, HashSet<usize>>,
    tagged_multi: HashMap<String, HashSet<usize>>,
}

/// Scan source lines and collect all conformance annotations by 1-based line
/// number.
fn collect_annotations(source: &str) -> Annotations {
    let mut required: HashSet<usize> = HashSet::new();
    let mut optional: HashSet<usize> = HashSet::new();
    let mut tagged_exact: HashMap<String, HashSet<usize>> = HashMap::new();
    let mut tagged_multi: HashMap<String, HashSet<usize>> = HashMap::new();

    for (idx, line) in source.lines().enumerate() {
        let lineno = idx + 1;
        match parse_annotation(line) {
            Some(Annotation::Required) => {
                let _ = required.insert(lineno);
            }
            Some(Annotation::Optional) => {
                let _ = optional.insert(lineno);
            }
            Some(Annotation::TaggedExact(tag)) => {
                let _ = tagged_exact.entry(tag).or_default().insert(lineno);
            }
            Some(Annotation::TaggedMulti(tag)) => {
                let _ = tagged_multi.entry(tag).or_default().insert(lineno);
            }
            None => {}
        }
    }

    Annotations {
        required,
        optional,
        tagged_exact,
        tagged_multi,
    }
}

// ---------------------------------------------------------------------------
// Diagnostic collection
// ---------------------------------------------------------------------------

struct DiagnosticOutput {
    diag_lines: HashSet<usize>,
    rules_seen: std::collections::BTreeSet<String>,
    diag_line_rules: HashMap<usize, Vec<String>>,
}

/// Run the Basilisk pipeline on `path` and collect diagnostic lines, filtering
/// out strictness-only rules.
fn collect_diagnostics(path: &Path, source: &str) -> DiagnosticOutput {
    // Rules that are Basilisk-specific strictness requirements not covered by
    // the PEP conformance suite.  These codes are excluded from both the
    // "caught" count and the false-positive count so they do not inflate or
    // deflate the conformance score:
    //
    // - E0001–E0005: annotation completeness (PEP suite fixtures are unannotated)
    // - E0010, E0011: import strictness and Any warnings
    // - E0023: non-exhaustive match — PEP conformance suite tests type narrowing
    //          inside match arms but does not require a wildcard `case _:` branch
    // - E0025: missing @override (PEP 698 makes @override optional documentation)
    const STRICTNESS_ONLY: &[&str] = &[
        "BSK-E0001",
        "BSK-E0002",
        "BSK-E0003",
        "BSK-E0004",
        "BSK-E0005",
        "BSK-E0010",
        "BSK-E0011",
        "BSK-E0023",
        "BSK-E0025",
    ];

    let mut rules_seen = std::collections::BTreeSet::new();
    let mut diag_line_rules: HashMap<usize, Vec<String>> = HashMap::new();

    let diag_lines: HashSet<usize> = match parse_file(path.to_string_lossy().as_ref()) {
        Ok(parsed) => match resolve(&parsed) {
            Ok(resolved) => {
                let diags = check(&resolved);
                diags
                    .iter()
                    .filter(|d| d.severity == basilisk_checker::Severity::Error)
                    .filter(|d| !STRICTNESS_ONLY.contains(&d.code.code))
                    .map(|d| {
                        let _ = rules_seen.insert(d.code.code.to_owned());
                        let line = byte_offset_to_line(source, d.span.start);
                        diag_line_rules
                            .entry(line)
                            .or_default()
                            .push(d.code.code.to_owned());
                        line
                    })
                    .collect()
            }
            Err(_) => HashSet::new(),
        },
        Err(_) => HashSet::new(),
    };

    DiagnosticOutput {
        diag_lines,
        rules_seen,
        diag_line_rules,
    }
}

// ---------------------------------------------------------------------------
// Run one conformance file
// ---------------------------------------------------------------------------

fn run_file(path: &Path) -> FileResult {
    let Ok(source) = fs::read_to_string(path) else {
        return FileResult::default();
    };

    let annotations = collect_annotations(&source);
    let diagnostics = collect_diagnostics(path, &source);

    // Score required lines.
    let caught = annotations
        .required
        .iter()
        .filter(|l| diagnostics.diag_lines.contains(l))
        .count();
    let missed = annotations.required.len() - caught;

    // Score optional lines.
    let optional_caught = annotations
        .optional
        .iter()
        .filter(|l| diagnostics.diag_lines.contains(l))
        .count();

    // Score tagged-exact groups: a group passes if at least one line errored.
    let mut tagged_exact_satisfied = 0usize;
    let mut tagged_exact_missed = 0usize;
    for lines in annotations.tagged_exact.values() {
        if lines.iter().any(|l| diagnostics.diag_lines.contains(l)) {
            tagged_exact_satisfied += 1;
        } else {
            tagged_exact_missed += 1;
        }
    }

    // All annotated lines (don't count false positives on annotated lines).
    let all_annotated: HashSet<usize> = annotations
        .required
        .iter()
        .chain(annotations.optional.iter())
        .chain(annotations.tagged_exact.values().flatten())
        .chain(annotations.tagged_multi.values().flatten())
        .copied()
        .collect();

    let false_positives = diagnostics
        .diag_lines
        .iter()
        .filter(|l| !all_annotated.contains(l))
        .count();

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    if missed > 0 {
        let missed_lines: Vec<usize> = annotations
            .required
            .iter()
            .filter(|l| !diagnostics.diag_lines.contains(l))
            .copied()
            .collect();
        println!("  DEBUG {file_name}: missed={missed} lines={missed_lines:?}");
    }
    if false_positives > 0 {
        let mut fp_details: Vec<(usize, String)> = diagnostics
            .diag_lines
            .iter()
            .filter(|l| !all_annotated.contains(l))
            .map(|&l| {
                let rules = diagnostics
                    .diag_line_rules
                    .get(&l)
                    .map_or_else(String::new, |codes| codes.join("|"));
                (l, rules)
            })
            .collect();
        fp_details.sort_by_key(|(l, _)| *l);
        println!("  FP    {file_name}: count={false_positives} lines={fp_details:?}");
    }

    FileResult {
        caught,
        missed,
        false_positives,
        optional_caught,
        tagged_exact_satisfied,
        tagged_exact_missed,
        rules_fired: diagnostics.rules_seen.into_iter().collect(),
    }
}

// ---------------------------------------------------------------------------
// Category from filename  (e.g. "generics_basic.py" → "generics")
// ---------------------------------------------------------------------------

fn category(name: &str) -> &str {
    name.find('_')
        .map_or(name.trim_end_matches(".py"), |i| &name[..i])
}

// ---------------------------------------------------------------------------
// The single test entry point
// ---------------------------------------------------------------------------

#[test]
fn conformance_score() {
    let conformance_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");

    if !conformance_dir.exists() {
        println!();
        println!("  ⚠  Conformance suite not downloaded.");
        println!("  Run: ./conformance/fetch-conformance.sh");
        println!("  Then rerun: cargo test --test conformance_tests -- --nocapture");
        println!();
        return;
    }

    let Ok(read_dir) = fs::read_dir(&conformance_dir) else {
        println!("  Failed to read conformance directory.");
        return;
    };
    let mut files: Vec<_> = read_dir
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "py"))
        .collect();
    files.sort_by_key(std::fs::DirEntry::file_name);

    if files.is_empty() {
        println!("  Conformance directory exists but contains no .py files.");
        println!("  Run: ./conformance/fetch-conformance.sh");
        return;
    }

    let (totals, by_category, detail_lines) = collect_results(&files);
    print_scorecard(&totals, &by_category, &detail_lines);
    write_csv(&detail_lines);

    assert!(
        totals.files > 0,
        "No conformance files found. Run ./conformance/fetch-conformance.sh first."
    );
}

type CategoryMap = BTreeMap<String, (usize, usize)>;
type DetailLines = Vec<(String, FileResult)>;

/// Aggregated conformance totals.
struct Totals {
    files: usize,
    pass: usize,
    caught: usize,
    missed: usize,
    fp: usize,
    tag_ok: usize,
    tag_missed: usize,
}

/// Write a CSV snapshot of per-file conformance results.
///
/// Output path: `conformance/conformance_status.csv` (repo root).
/// Columns: file, category, status, caught, missed, `false_positives`
///
/// This file is the rolling log — commit it after each run to track regressions.
fn write_csv(detail_lines: &DetailLines) {
    use std::fmt::Write;

    // Walk up from the manifest dir to find the workspace root (contains both
    // Cargo.toml and a `crates/` subdirectory — distinguishes it from crate-level Cargo.toml).
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(repo_root) = manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
    else {
        eprintln!("  [conformance csv] could not locate repo root");
        return;
    };
    let csv_path = repo_root.join("conformance/conformance_status.csv");
    let _ = fs::create_dir_all(csv_path.parent().unwrap_or(Path::new(".")));

    let mut out =
        String::from("basilisk_rules,file,category,status,caught,missed,false_positives\n");
    for (name, result) in detail_lines {
        let cat = category(name);
        let status = if result.passes() { "PASS" } else { "FAIL" };
        let rules = result.rules_fired.join("|");
        let _ = writeln!(
            out,
            "{rules},{name},{cat},{status},{},{},{}",
            result.caught, result.missed, result.false_positives
        );
    }

    match fs::write(&csv_path, &out) {
        Ok(()) => println!("  Conformance CSV: {}", csv_path.display()),
        Err(e) => eprintln!("  [conformance csv] write failed: {e}"),
    }
}

fn collect_results(files: &[std::fs::DirEntry]) -> (Totals, CategoryMap, DetailLines) {
    let mut by_category: CategoryMap = BTreeMap::new();
    let mut detail_lines: DetailLines = Vec::new();
    let mut totals = Totals {
        files: 0,
        pass: 0,
        caught: 0,
        missed: 0,
        fp: 0,
        tag_ok: 0,
        tag_missed: 0,
    };

    for entry in files {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let result = run_file(&path);
        let cat = category(&name).to_owned();
        let counts = by_category.entry(cat).or_insert((0, 0));
        counts.1 += 1;
        if result.passes() {
            counts.0 += 1;
            totals.pass += 1;
        }
        totals.files += 1;
        totals.caught += result.caught;
        totals.missed += result.missed;
        totals.fp += result.false_positives;
        totals.tag_ok += result.tagged_exact_satisfied;
        totals.tag_missed += result.tagged_exact_missed;
        detail_lines.push((name, result));
    }
    (totals, by_category, detail_lines)
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "percentage display requires float conversion from counters"
)]
fn print_scorecard(t: &Totals, by_category: &CategoryMap, detail_lines: &DetailLines) {
    let pct = if t.files > 0 {
        (t.pass as f64 / t.files as f64) * 100.0
    } else {
        0.0
    };
    let fail = t.files - t.pass;
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           BASILISK PEP CONFORMANCE SCORECARD                 ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Files:    {:>4} total │ {:>4} pass │ {fail:>4} fail            ║",
        t.files, t.pass
    );
    println!("║  Score:    {pct:.1}%                                           ║");
    println!(
        "║  Required: {:>4} caught │ {:>4} missed                       ║",
        t.caught, t.missed
    );
    println!(
        "║  Tagged:   {:>4} groups ok │ {:>4} groups missed              ║",
        t.tag_ok, t.tag_missed
    );
    println!(
        "║  False+:   {:>4} unexpected diagnostics                       ║",
        t.fp
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Category breakdown                                          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    for (cat, (pass, total)) in by_category {
        let cat_pct = if *total > 0 {
            (*pass as f64 / *total as f64) * 100.0
        } else {
            0.0
        };
        let bar_filled = (cat_pct / 5.0).round() as usize;
        let bar = format!("{}{}", "█".repeat(bar_filled), "░".repeat(20 - bar_filled));
        println!("║  {cat:<22} {pass:>2}/{total:<2}  {cat_pct:>5.1}%  {bar}  ║");
    }
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Failing files                                               ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    let mut any_fail = false;
    for (name, result) in detail_lines {
        if !result.passes() {
            any_fail = true;
            println!(
                "║  ✗ {:<57} ║",
                format!(
                    "{name} (missed {}, fp {})",
                    result.missed, result.false_positives
                )
            );
        }
    }
    if !any_fail {
        println!("║  (none — all files pass)                                     ║");
    }
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
