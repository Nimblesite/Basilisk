// Eleventy global data: PEP conformance results, computed FRESH at every build
// from the committed outputs of the REAL python/typing calculator — never
// hand-typed. Implements [CHKARCH-CONFORMANCE]; mirrors _data/benchmarks.js.
//
//   conformance/conformance_status.csv  -> live per-file pass/fail (score.py)
//   conformance/score.py                -> pinned upstream ref + sha256
//   conformance/upstream_main.py        -> re-hashed here to re-verify the pin
//   git log of conformance_status.csv   -> the over-time chart (real commits)
//
// A file passes iff the official calculator's `errors_diff` is empty. Every
// number the website shows is whatever that scorer last produced and committed —
// and the over-time chart is read straight from this file's GIT history, not a
// hand-maintained ledger, so it cannot drift from what actually happened.
import { readFileSync, existsSync, statSync } from "fs";
import { execFileSync } from "child_process";
import { createHash } from "crypto";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, "../../..");
const CONF_DIR = join(REPO_ROOT, "conformance");
const STATUS_REL = "conformance/conformance_status.csv";
const STATUS_CSV = join(CONF_DIR, "conformance_status.csv");
const SCORE_PY = join(CONF_DIR, "score.py");
const UPSTREAM_MAIN = join(CONF_DIR, "upstream_main.py");

// The day the REAL python/typing calculator replaced the rigged in-repo harness
// (the harness excluded 9 diagnostic codes and ignored false positives, inflating
// the score to a fake 100%). Commits dated on/after this used the official scorer.
const OFFICIAL_SINCE = "2026-06-23";

// The CSV stores lowercase category slugs; these render the few that are not a
// plain title-case word. Everything else falls back to capitalising the slug.
const CATEGORY_LABELS = {
  typeddicts: "TypedDicts",
  namedtuples: "NamedTuples",
  typeforms: "TypeForms",
  specialtypes: "Special types",
};

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const round1 = (n) => Math.round(n * 10) / 10;
const labelFor = (slug) => CATEGORY_LABELS[slug] || (slug ? slug.charAt(0).toUpperCase() + slug.slice(1) : "—");

// "2026-06-21" -> "Jun 21" (manual parse — no timezone surprises).
function shortDate(iso) {
  const [, m, d] = iso.split("-").map((p) => parseInt(p, 10));
  return Number.isFinite(m) && Number.isFinite(d) ? `${MONTHS[m - 1]} ${d}` : iso;
}

// Pull a `NAME = "value"` string constant straight out of score.py so the pin
// shown on the website is the exact one the scorer enforces, not a copy.
function constFromScorePy(name) {
  if (!existsSync(SCORE_PY)) return null;
  const src = readFileSync(SCORE_PY, "utf-8");
  const m = src.match(new RegExp(`^${name}\\s*=\\s*"([^"]+)"`, "m"));
  return m ? m[1] : null;
}

// Tally one CSV body (pass/total/fp/missed) from its raw text.
function tally(csvText) {
  const rows = csvText.split(/\r?\n/).slice(1).filter((l) => l.trim() && !l.startsWith("#"));
  const t = { pass: 0, total: 0, fp: 0, missed: 0, byFile: rows };
  for (const line of rows) {
    const f = line.split(",");
    if (f.length < 7) continue;
    t.total += 1;
    if (f[3] === "PASS") t.pass += 1;
    t.missed += parseInt(f[5], 10) || 0;
    t.fp += parseInt(f[6], 10) || 0;
  }
  return t;
}

function parseStatus() {
  if (!existsSync(STATUS_CSV)) return null;
  const text = readFileSync(STATUS_CSV, "utf-8");
  const t = tally(text);
  if (!t.total) return null;

  const cats = new Map();
  const failing = [];
  let caught = 0;
  for (const line of t.byFile) {
    const f = line.split(",");
    if (f.length < 7) continue;
    const passed = f[3] === "PASS";
    const slug = f[2];
    const missed = parseInt(f[5], 10) || 0;
    const fp = parseInt(f[6], 10) || 0;
    caught += parseInt(f[4], 10) || 0;
    if (!cats.has(slug)) cats.set(slug, { slug, label: labelFor(slug), pass: 0, total: 0 });
    const entry = cats.get(slug);
    entry.total += 1;
    entry.pass += passed ? 1 : 0;
    if (!passed) failing.push({ file: f[1], category: slug, missed, fp });
  }

  const categories = [...cats.values()]
    .filter((c) => c.slug)
    .map((c) => ({ ...c, pct: round1((c.pass / c.total) * 100) }))
    .sort((a, b) => a.label.localeCompare(b.label));

  return {
    pass: t.pass,
    total: t.total,
    fail: t.total - t.pass,
    caught,
    missed: t.missed,
    fp: t.fp,
    scorePct: round1((t.pass / t.total) * 100),
    categories,
    categoriesTotal: categories.length,
    categoriesPass100: categories.filter((c) => c.pass === c.total).length,
    failing: failing.sort((a, b) => b.fp + b.missed - (a.fp + a.missed)),
  };
}

function git(args) {
  // stderr ignored: early commits hold the file under an old path, so `git show`
  // legitimately fails for those — we skip them, no need to spam the build log.
  return execFileSync("git", args, { cwd: REPO_ROOT, encoding: "utf-8", maxBuffer: 1 << 26, stdio: ["ignore", "pipe", "ignore"] });
}

// The over-time series, read from the GIT history of conformance_status.csv.
// One real data point per commit that changed the file: its commit date and the
// score that commit recorded. Points dated before OFFICIAL_SINCE were produced
// by the rigged harness; on/after, by the official calculator.
function gitHistory() {
  let log;
  try {
    log = git(["log", "--follow", "--format=%H|%cs", "--", STATUS_REL]);
  } catch {
    return [];
  }
  const points = [];
  for (const line of log.split(/\r?\n/).filter(Boolean)) {
    const [hash, date] = line.split("|");
    let csv;
    try {
      csv = git(["show", `${hash}:${STATUS_REL}`]);
    } catch {
      continue;
    }
    const t = tally(csv);
    if (!t.total) continue;
    points.push({
      hash: hash.slice(0, 8),
      date,
      shortDate: shortDate(date),
      pass: t.pass,
      total: t.total,
      fp: t.fp,
      missed: t.missed,
      score: round1((t.pass / t.total) * 100),
      official: date >= OFFICIAL_SINCE,
    });
  }
  return points.reverse(); // oldest -> newest
}

// Inline-SVG geometry for the over-time chart. Computed here (testable, DRY) so
// the Nunjucks include only loops over coordinates. Points are spaced evenly by
// commit (each is a real event); the y-axis is the pass percentage 0–100.
function buildChart(points) {
  if (points.length < 2) return null;
  const width = 760, height = 360, left = 48, right = 24, top = 28, bottom = 64;
  const plotW = width - left - right, plotH = height - top - bottom;
  const n = points.length;
  const xAt = (i) => round1(left + (i / (n - 1)) * plotW);
  const yAt = (score) => round1(top + (1 - score / 100) * plotH);

  let lastLabel = null;
  const pts = points.map((p, i) => {
    const showDate = p.shortDate !== lastLabel;
    lastLabel = p.shortDate;
    return { ...p, i, x: xAt(i), y: yAt(p.score), showDate };
  });
  const yTicks = [0, 25, 50, 75, 100].map((value) => ({ value, y: yAt(value) }));

  const rigged = pts.filter((p) => !p.official);
  const official = pts.filter((p) => p.official);
  const lastRigged = rigged[rigged.length - 1];
  const firstOfficial = official[0];
  const peak = pts.reduce((a, b) => (b.score > a.score ? b : a), pts[0]);

  return {
    width, height, left, right, top, bottom,
    baselineY: yAt(0),
    pts,
    yTicks,
    riggedPolyline: rigged.map((p) => `${p.x},${p.y}`).join(" "),
    officialPolyline: official.map((p) => `${p.x},${p.y}`).join(" "),
    // The correction "cliff": last rigged point down to the first official one.
    drop: lastRigged && firstOfficial
      ? { x1: lastRigged.x, y1: lastRigged.y, x2: firstOfficial.x, y2: firstOfficial.y, from: lastRigged.score, to: firstOfficial.score }
      : null,
    peak,
    current: pts[pts.length - 1],
  };
}

export default function () {
  const status = parseStatus();
  if (!status) {
    return { hasData: false, scorePct: null, categories: [], failing: [], history: [], chart: null };
  }

  const pinnedRef = constFromScorePy("PINNED_TYPING_REF");
  const sha256 = constFromScorePy("UPSTREAM_MAIN_SHA256");
  // Re-verify the committed calculator at build time — the page states this.
  let liveSha = null, upstreamBytes = null, verified = false;
  if (existsSync(UPSTREAM_MAIN)) {
    const raw = readFileSync(UPSTREAM_MAIN);
    liveSha = createHash("sha256").update(raw).digest("hex");
    upstreamBytes = statSync(UPSTREAM_MAIN).size;
    verified = sha256 != null && liveSha === sha256;
  }

  const history = gitHistory();
  return {
    hasData: true,
    ...status,
    pinnedRef,
    sha256,
    sha256Short: sha256 ? sha256.slice(0, 12) : null,
    liveSha256Short: liveSha ? liveSha.slice(0, 12) : null,
    verified,
    upstreamBytes,
    officialSince: OFFICIAL_SINCE,
    history,
    chart: buildChart(history),
  };
}
