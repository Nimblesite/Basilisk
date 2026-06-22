//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! PEP conformance test harness — faithful port of the **official** scoring.
//!
//! Runs every `.py` file from the `python/typing` conformance suite against the
//! Basilisk pipeline and scores it with the **exact** algorithm the upstream
//! `python/typing` tool uses (`conformance/src/main.py`,
//! `get_expected_errors` + `diff_expected_errors`).  There are **no
//! Basilisk-specific scoring rules** and **no excluded diagnostic codes** — a
//! file passes iff the official `errors_diff` is empty.
//!
//! ## Prerequisites
//!
//! The conformance files must be downloaded first:
//!
//! ```text
//! make conformance          # fetch if needed + run
//! make conformance FETCH=1  # force re-download + run
//! ```
//!
//! ## Annotation format (verbatim from `python/typing`)
//!
//! For every source line, the upstream tool first strips the comment
//! (`line.split('#')[0]`); if nothing but whitespace precedes the first `#`,
//! the whole line is **ignored** (this is how commented-out cases are skipped).
//! Otherwise it scans the *raw* line for these markers:
//!
//! | Marker      | Regex (upstream)            | Meaning                                  |
//! |-------------|-----------------------------|------------------------------------------|
//! | `# E`       | `# E\??(?=:\|$\| )`          | An error MUST be reported on this line   |
//! | `# E?`      | `# E\??(?=:\|$\| )`          | An error MAY be reported (optional)      |
//! | `# E[tag]`  | `# E\[([^\]]+)\]`            | Exactly one line in the group must error |
//! | `# E[tag+]` | `# E\[([^\]]+)\]`            | One or more lines in the group may error |
//!
//! The `(?=:|$| )` lookahead means the marker must be followed by `:`, end of
//! line, or a space — so `# Exception` and `# E0001` do **not** match.
//!
//! ## Scoring (official `diff_expected_errors`)
//!
//! A file's `errors_diff` collects three kinds of discrepancy:
//!
//! 1. **Missed required** — a `# E` line where Basilisk reported no error.
//! 2. **Missed tag group** — a `# E[tag]` group where no line errored (or, for
//!    the non-`+` form, more than one line errored).
//! 3. **Unexpected error** — Basilisk reported an error on a line carrying
//!    neither a `# E`/`# E?` marker nor a satisfied tag-group line.  These are
//!    the **false positives**, and — unlike the previous in-repo harness — they
//!    **fail the file**, exactly as upstream does
//!    (`conformance_automated = "Fail" if errors_diff.strip() else "Pass"`).
//!
//! Every `Severity::Error` diagnostic Basilisk emits is counted; **no code is
//! excluded**.  This is the same number a user sees from `basilisk check`.
//!
//! ## Skip behaviour
//!
//! If the conformance directory does not exist the test prints a clear message
//! and exits with success so that CI on a fresh checkout does not break.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
};

use basilisk_checker::check;
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

// ---------------------------------------------------------------------------
// Expected-error parsing — faithful port of `get_expected_errors` (main.py)
// ---------------------------------------------------------------------------

/// Expected-error annotations parsed from one conformance file.
struct Expected {
    /// 1-based line → (required count, optional count). A line is present iff
    /// it carries at least one `# E` or `# E?` marker.
    lines: HashMap<usize, (u32, u32)>,
    /// tag → (line numbers carrying the tag, `allow_multiple`).
    groups: HashMap<String, (Vec<usize>, bool)>,
}

/// Apply the upstream `(?=:|$| )` lookahead: the char immediately after the
/// marker must be `:`, a space, or the end of the line.
fn lookahead_ok(after: &str) -> bool {
    matches!(after.chars().next(), None | Some(':') | Some(' '))
}

/// Count `# E` (required) and `# E?` (optional) markers on a line, matching the
/// upstream regex `# E\??(?=:|$| )` exactly.
fn count_markers(line: &str) -> (u32, u32) {
    let (mut required, mut optional) = (0u32, 0u32);
    let mut search_from = 0usize;
    while let Some(rel) = line[search_from..].find("# E") {
        let idx = search_from + rel;
        let after = &line[idx + 3..]; // chars after "# E"
        if let Some(rest) = after.strip_prefix('?') {
            // `\??` greedily consumed the `?`; lookahead applies to what follows.
            if lookahead_ok(rest) {
                optional += 1;
            }
        } else if lookahead_ok(after) {
            required += 1;
        }
        // Advance past this "# E" occurrence (upstream finditer is non-overlapping).
        search_from = idx + 3;
    }
    (required, optional)
}

/// Parse `# E[tag]` / `# E[tag+]` groups on a line, matching the upstream regex
/// `# E\[([^\]]+)\]` exactly.
fn parse_groups(line: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = line[search_from..].find("# E[") {
        let open = search_from + rel + "# E[".len();
        let Some(close_rel) = line[open..].find(']') else {
            break;
        };
        let inner = &line[open..open + close_rel];
        if !inner.is_empty() {
            let (tag, allow_multiple) = inner
                .strip_suffix('+')
                .map_or((inner, false), |stripped| (stripped, true));
            out.push((tag.to_owned(), allow_multiple));
        }
        search_from = open + close_rel + 1;
    }
    out
}

/// Faithful port of upstream `get_expected_errors`.
fn get_expected_errors(source: &str) -> Expected {
    let mut lines: HashMap<usize, (u32, u32)> = HashMap::new();
    let mut groups: HashMap<String, (Vec<usize>, bool)> = HashMap::new();

    for (idx, line) in source.lines().enumerate() {
        let lineno = idx + 1;
        // `line.split('#')[0]` — skip lines with no code before the first '#'
        // (this is how upstream ignores commented-out test cases).
        let before_hash = line.split('#').next().unwrap_or("");
        if before_hash.trim().is_empty() {
            continue;
        }

        let (required, optional) = count_markers(line);
        if required > 0 || optional > 0 {
            let _ = lines.insert(lineno, (required, optional));
        }

        for (tag, allow_multiple) in parse_groups(line) {
            let entry = groups.entry(tag).or_insert_with(|| (Vec::new(), allow_multiple));
            entry.0.push(lineno);
        }
    }

    Expected { lines, groups }
}

// ---------------------------------------------------------------------------
// Diagnostic collection — every Severity::Error, NO exclusions
// ---------------------------------------------------------------------------

/// Line numbers (1-based) where Basilisk reported an `Error`, with the codes
/// that fired there. This is exactly what `basilisk check` prints — no code is
/// filtered out.
struct Diagnostics {
    by_line: HashMap<usize, Vec<String>>,
    rules_seen: BTreeSet<String>,
}

fn byte_offset_to_line(source: &str, offset: u32) -> usize {
    let clamped = (offset as usize).min(source.len());
    source[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

fn collect_diagnostics(path: &Path, source: &str) -> Diagnostics {
    let mut by_line: HashMap<usize, Vec<String>> = HashMap::new();
    let mut rules_seen = BTreeSet::new();

    if let Ok(parsed) = parse_file(path.to_string_lossy().as_ref()) {
        if let Ok(resolved) = resolve(&parsed) {
            for diag in check(&resolved)
                .iter()
                .filter(|d| d.severity == basilisk_checker::Severity::Error)
            {
                let _ = rules_seen.insert(diag.code.code.to_owned());
                let line = byte_offset_to_line(source, diag.span.start);
                by_line.entry(line).or_default().push(diag.code.code.to_owned());
            }
        }
    }

    Diagnostics { by_line, rules_seen }
}

// ---------------------------------------------------------------------------
// The official diff — faithful port of `diff_expected_errors` (main.py)
// ---------------------------------------------------------------------------

/// One scored conformance file.
#[derive(Debug, Default)]
struct FileResult {
    /// Required lines Basilisk caught.
    required_caught: usize,
    /// `# E` lines + tag groups Basilisk missed (false negatives).
    missed: usize,
    /// Lines Basilisk flagged that no annotation expected (false positives).
    false_positives: usize,
    /// Distinct Basilisk codes that fired on this file.
    rules_fired: Vec<String>,
    /// The upstream-style discrepancy strings (empty ⇒ Pass).
    diffs: Vec<String>,
}

impl FileResult {
    /// A file passes iff the official `errors_diff` is empty.
    fn passes(&self) -> bool {
        self.diffs.is_empty()
    }
}

fn run_file(path: &Path) -> FileResult {
    let Ok(source) = fs::read_to_string(path) else {
        return FileResult::default();
    };

    let expected = get_expected_errors(&source);
    let diagnostics = collect_diagnostics(path, &source);
    let errors = &diagnostics.by_line;

    let mut diffs: Vec<String> = Vec::new();
    let mut missed = 0usize;
    let mut false_positives = 0usize;

    // 1. Missed required lines.
    let mut required_caught = 0usize;
    for (&lineno, &(required, _optional)) in &expected.lines {
        if required > 0 {
            if errors.contains_key(&lineno) {
                required_caught += 1;
            } else {
                missed += 1;
                diffs.push(format!("Line {lineno}: Expected {required} errors"));
            }
        }
    }

    // 2. Tag groups (and the set of group lines that "absorb" an error so they
    //    are not later counted as unexpected).
    let mut linenos_used_by_groups: HashSet<usize> = HashSet::new();
    for (tag, (linenos, allow_multiple)) in &expected.groups {
        let num_errors = linenos.iter().filter(|l| errors.contains_key(l)).count();
        if num_errors == 0 {
            missed += 1;
            diffs.push(format!("Lines {linenos:?}: Expected error (tag {tag:?})"));
        } else if num_errors == 1 || *allow_multiple {
            linenos_used_by_groups.extend(linenos.iter().copied());
        } else {
            missed += 1;
            diffs.push(format!("Lines {linenos:?}: Expected exactly one error (tag {tag:?})"));
        }
    }

    // 3. Unexpected errors (false positives).
    let mut fp_lines: Vec<(usize, String)> = Vec::new();
    for (&lineno, codes) in errors {
        if !expected.lines.contains_key(&lineno) && !linenos_used_by_groups.contains(&lineno) {
            false_positives += 1;
            fp_lines.push((lineno, codes.join("|")));
            diffs.push(format!("Line {lineno}: Unexpected errors {codes:?}"));
        }
    }

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    if missed > 0 || false_positives > 0 {
        fp_lines.sort_by_key(|(l, _)| *l);
        println!(
            "  {file_name}: missed={missed} fp={false_positives} fp_lines={fp_lines:?}"
        );
    }

    FileResult {
        required_caught,
        missed,
        false_positives,
        rules_fired: diagnostics.rules_seen.into_iter().collect(),
        diffs,
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
// Thresholds from coverage-thresholds.json
// ---------------------------------------------------------------------------

/// Read the PEP conformance pass-percentage threshold (ratchets UP only).
fn read_conformance_threshold() -> usize {
    read_conformance_field("threshold").unwrap_or(0)
}

/// Read the maximum total false positives allowed across the suite (ratchets
/// DOWN only). `None` ⇒ gate disabled.
fn read_conformance_fp_ceiling() -> Option<usize> {
    read_conformance_field("max_false_positives")
}

/// Read a numeric field nested under the `"conformance"` object in
/// `coverage-thresholds.json` (minimal extraction — no serde in this crate).
fn read_conformance_field(key: &str) -> Option<usize> {
    let repo_root = repo_root()?;
    let content = fs::read_to_string(repo_root.join("coverage-thresholds.json")).ok()?;
    let conformance_idx = content.find("\"conformance\"")?;
    let rest = &content[conformance_idx..];
    let key_pat = format!("\"{key}\"");
    let key_idx = rest.find(&key_pat)?;
    let after = &rest[key_idx + key_pat.len()..];
    let num_start = after.find(|c: char| c.is_ascii_digit())?;
    let num_end = after[num_start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(after.len(), |i| num_start + i);
    after[num_start..num_end].parse().ok()
}

/// Walk up from the manifest dir to the workspace root (has both `Cargo.toml`
/// and a `crates/` subdirectory).
fn repo_root() -> Option<&'static Path> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
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
        println!("  Run: make conformance");
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
        println!("  Run: make conformance");
        return;
    }

    let (totals, by_category, detail_lines) = collect_results(&files);
    print_scorecard(&totals, &by_category, &detail_lines);
    write_csv(&detail_lines);

    assert!(
        totals.files > 0,
        "No conformance files found. Run make conformance first."
    );

    // Pass-percentage gate (ratchets UP only). This is the OFFICIAL pass rate:
    // files with an empty errors_diff over total files.
    let threshold = read_conformance_threshold();
    let pct = (totals.pass * 100).checked_div(totals.files).unwrap_or(0);
    assert!(
        pct >= threshold,
        "PEP conformance regression: {pct}% ({}/{}) < {threshold}% threshold. \
         Fix the regression before merging.",
        totals.pass,
        totals.files
    );
    println!(
        "  Conformance gate: {pct}% ({}/{}) >= {threshold}% threshold — PASS",
        totals.pass, totals.files
    );

    // False-positive ceiling (ratchets DOWN only).
    if let Some(ceiling) = read_conformance_fp_ceiling() {
        assert!(
            totals.fp <= ceiling,
            "PEP conformance false-positive regression: {} FPs > {ceiling} ceiling. \
             False positives ratchet DOWN only — eliminate new ones before merging.",
            totals.fp
        );
        println!("  FP gate: {} <= {ceiling} ceiling — PASS", totals.fp);
    }
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
}

/// Write a CSV snapshot of per-file conformance results to
/// `conformance/conformance_status.csv` (repo root).
fn write_csv(detail_lines: &DetailLines) {
    use std::fmt::Write;

    let Some(repo_root) = repo_root() else {
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
            result.required_caught, result.missed, result.false_positives
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
        totals.caught += result.required_caught;
        totals.missed += result.missed;
        totals.fp += result.false_positives;
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
    println!("║      BASILISK PEP CONFORMANCE SCORECARD (OFFICIAL SCORING)    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Files:    {:>4} total │ {:>4} pass │ {fail:>4} fail            ║",
        t.files, t.pass
    );
    println!("║  Score:    {pct:.1}%  (empty errors_diff = Pass, upstream rule) ║");
    println!(
        "║  Required: {:>4} caught │ {:>4} missed                       ║",
        t.caught, t.missed
    );
    println!(
        "║  False+:   {:>4} unexpected diagnostics (THESE FAIL FILES)    ║",
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
