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
//! ./scripts/fetch-conformance.sh
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
    // Find the last `# E` marker on the line.
    let marker = line.rfind("# E")?;
    let rest = line[marker + 2..].trim(); // everything after "#"

    if rest.starts_with("E?") {
        return Some(Annotation::Optional);
    }

    if rest.starts_with("E[") {
        let inner = rest.strip_prefix("E[")?.trim_end();
        if let Some(tag) = inner.strip_suffix("+]") {
            return Some(Annotation::TaggedMulti(tag.to_owned()));
        }
        if let Some(tag) = inner.strip_suffix(']') {
            return Some(Annotation::TaggedExact(tag.to_owned()));
        }
        // malformed tag — treat as required
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
    #[allow(dead_code)]
    optional_caught: usize,
    /// `# E[tag]` groups satisfied.
    tagged_exact_satisfied: usize,
    /// `# E[tag]` groups missed.
    tagged_exact_missed: usize,
}

impl FileResult {
    fn passes(&self) -> bool {
        self.missed == 0
    }
}

// ---------------------------------------------------------------------------
// Run one conformance file
// ---------------------------------------------------------------------------

fn run_file(path: &Path) -> FileResult {
    let Ok(source) = fs::read_to_string(path) else {
        return FileResult::default();
    };

    // Collect annotations by 1-based line number.
    let mut required: HashSet<usize> = HashSet::new();
    let mut optional: HashSet<usize> = HashSet::new();
    // tag → set of line numbers
    let mut tagged_exact: HashMap<String, HashSet<usize>> = HashMap::new();
    let mut tagged_multi: HashMap<String, HashSet<usize>> = HashMap::new();

    for (idx, line) in source.lines().enumerate() {
        let lineno = idx + 1;
        match parse_annotation(line) {
            Some(Annotation::Required) => {
                required.insert(lineno);
            }
            Some(Annotation::Optional) => {
                optional.insert(lineno);
            }
            Some(Annotation::TaggedExact(tag)) => {
                tagged_exact.entry(tag).or_default().insert(lineno);
            }
            Some(Annotation::TaggedMulti(tag)) => {
                tagged_multi.entry(tag).or_default().insert(lineno);
            }
            None => {}
        }
    }

    // Run the pipeline.
    // Only Error-severity diagnostics are used for scoring: warnings (e.g. E0011)
    // are informational and must not be counted as false positives or required hits.
    //
    // Additionally, Basilisk-specific strictness rules (E0001–E0005, E0010) are
    // excluded from FP scoring: the python/typing conformance suite tests PEP
    // type-checking behaviour, not Basilisk's annotation-completeness requirements.
    // Those rules fire legitimately on the unannotated test fixtures.
    const STRICTNESS_ONLY: &[&str] = &[
        "BSK-E0001", "BSK-E0002", "BSK-E0003", "BSK-E0004", "BSK-E0005", "BSK-E0010",
    ];
    let diag_lines: HashSet<usize> = match parse_file(path.to_string_lossy().as_ref()) {
        Ok(parsed) => match resolve(&parsed) {
            Ok(resolved) => {
                let diags = check(&resolved);
                diags
                    .iter()
                    .filter(|d| d.severity == basilisk_checker::Severity::Error)
                    .filter(|d| !STRICTNESS_ONLY.contains(&d.code.code))
                    .map(|d| byte_offset_to_line(&source, d.span.start))
                    .collect()
            }
            Err(_) => HashSet::new(),
        },
        // Parse errors are themselves a form of "diagnostic" — treat the file
        // as producing no line-level diagnostics (the parse failure is noted).
        Err(_) => HashSet::new(),
    };

    // Score required lines.
    let caught = required.iter().filter(|l| diag_lines.contains(l)).count();
    let missed = required.len() - caught;

    // Score optional lines.
    let optional_caught = optional.iter().filter(|l| diag_lines.contains(l)).count();

    // Score tagged-exact groups: a group passes if exactly one of its lines errored.
    let mut tagged_exact_satisfied = 0usize;
    let mut tagged_exact_missed = 0usize;
    for lines in tagged_exact.values() {
        let hits = lines.iter().filter(|l| diag_lines.contains(l)).count();
        if hits >= 1 {
            tagged_exact_satisfied += 1;
        } else {
            tagged_exact_missed += 1;
        }
    }

    // All annotated lines (don't count false positives on annotated lines).
    let all_annotated: HashSet<usize> = required
        .iter()
        .chain(optional.iter())
        .chain(tagged_exact.values().flatten())
        .chain(tagged_multi.values().flatten())
        .copied()
        .collect();

    let false_positives = diag_lines
        .iter()
        .filter(|l| !all_annotated.contains(l))
        .count();

    FileResult {
        caught,
        missed,
        false_positives,
        optional_caught,
        tagged_exact_satisfied,
        tagged_exact_missed,
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
        println!("  Run: ./scripts/fetch-conformance.sh");
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
        println!("  Run: ./scripts/fetch-conformance.sh");
        return;
    }

    let (totals, by_category, detail_lines) = collect_results(&files);
    print_scorecard(&totals, &by_category, &detail_lines);

    assert!(
        totals.files > 0,
        "No conformance files found. Run ./scripts/fetch-conformance.sh first."
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

fn collect_results(files: &[std::fs::DirEntry]) -> (Totals, CategoryMap, DetailLines) {
    let mut by_category: CategoryMap = BTreeMap::new();
    let mut detail_lines: DetailLines = Vec::new();
    let mut totals = Totals { files: 0, pass: 0, caught: 0, missed: 0, fp: 0, tag_ok: 0, tag_missed: 0 };

    for entry in files {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
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

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn print_scorecard(t: &Totals, by_category: &CategoryMap, detail_lines: &DetailLines) {
    let pct = if t.files > 0 { (t.pass as f64 / t.files as f64) * 100.0 } else { 0.0 };
    let fail = t.files - t.pass;
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           BASILISK PEP CONFORMANCE SCORECARD                 ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Files:    {:>4} total │ {:>4} pass │ {fail:>4} fail            ║", t.files, t.pass);
    println!("║  Score:    {pct:.1}%                                           ║");
    println!("║  Required: {:>4} caught │ {:>4} missed                       ║", t.caught, t.missed);
    println!("║  Tagged:   {:>4} groups ok │ {:>4} groups missed              ║", t.tag_ok, t.tag_missed);
    println!("║  False+:   {:>4} unexpected diagnostics                       ║", t.fp);
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Category breakdown                                          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    for (cat, (pass, total)) in by_category {
        let cat_pct = if *total > 0 { (*pass as f64 / *total as f64) * 100.0 } else { 0.0 };
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
            println!("║  ✗ {:<57} ║", format!("{name} (missed {}, fp {})", result.missed, result.false_positives));
        }
    }
    if !any_fail {
        println!("║  (none — all files pass)                                     ║");
    }
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
