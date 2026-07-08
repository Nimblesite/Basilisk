// Eleventy global data: benchmark results, read from the git-tracked per-machine
// CSV that `make bench` generates (benchmarks/status/<machine>.csv).
//
// The homepage benchmark table renders from this data, so the published numbers
// are always whatever was last measured + committed — never hand-typed.
//
// Primary machine selection (what the website shows):
//   1. $BASILISK_BENCH_PRIMARY (slug)   2. benchmarks/status/.primary file
//   3. otherwise rank by tool coverage (a CSV missing competitor columns must
//      never win), then prefer `gha-*` (stable CI hardware), then alphabetical
import { readFileSync, readdirSync, existsSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const STATUS_DIR = join(__dirname, "../../../benchmarks/status");

const titleCase = (s) => s.charAt(0).toUpperCase() + s.slice(1);

// A benchmark fixture's filename IS the typing-spec construct it stresses, so
// the human-readable row label is derived straight from the stem — never a
// tool-specific error code. "typeddict_key_access" -> "TypedDict key access".
// Sentence case, with a small map that preserves the casing of typing-spec
// proper nouns; no hand-maintained per-fixture list, so the label can never
// drift from the fixture set. The benchmark table macro is shared across
// locales — non-English pages pass a translated name map keyed by `fixture`,
// falling back to this English `name`.
const CONSTRUCT_ACRONYMS = {
  typeddict: "TypedDict",
  typevar: "TypeVar",
  typevars: "TypeVars",
  typeis: "TypeIs",
  newtype: "NewType",
  classvar: "ClassVar",
};
function fixtureName(stem) {
  const label = stem
    .split("_")
    .map((word) => CONSTRUCT_ACRONYMS[word] || word)
    .join(" ");
  return label.charAt(0).toUpperCase() + label.slice(1);
}

// Parse the CSV `# tools:` header into [{ tool, version }] for the methodology
// footnote. The header looks like:
//   "basilisk=basilisk 0.0.0, pyright=pyright 1.1.408, mypy=mypy 1.19.1 (compiled: yes), ..."
// i.e. comma-separated `name=<--version output>` entries. The version output
// usually repeats the tool name and may carry a trailing parenthetical, both of
// which we strip so the site shows a clean "pyright 1.1.408".
function parseToolVersions(toolsStr) {
  if (!toolsStr) return [];
  return toolsStr
    .split(/,\s+(?=[a-z0-9_]+=)/i)
    .map((entry) => {
      const eq = entry.indexOf("=");
      const tool = (eq >= 0 ? entry.slice(0, eq) : entry).trim();
      let version = (eq >= 0 ? entry.slice(eq + 1) : "").replace(/\s*\(.*\)\s*$/, "").trim();
      const prefix = `${tool} `;
      if (version.toLowerCase().startsWith(prefix.toLowerCase())) {
        version = version.slice(prefix.length).trim();
      }
      // Dev builds don't have a released version number. `make bench` pins them
      // to the source commit (0.0.0-dev+g<sha>); render that as "dev (<sha>)" so
      // the site still says exactly which build was measured. A bare placeholder
      // (no commit pin) degrades to a plain "dev build" label.
      const devPin = version.match(/dev\+g([0-9a-f]+(?:-dirty)?)/i);
      if (devPin) version = `dev (${devPin[1]})`;
      else if (!version || /placeholder/i.test(version)) version = "dev build";
      return { tool, version };
    })
    .filter((t) => t.tool);
}

function parseCsv(text) {
  const meta = {};
  const dataLines = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("#")) {
      const m = line.slice(1).match(/^\s*([^:]+):\s*(.*)$/);
      if (m) meta[m[1].trim()] = m[2].trim();
    } else {
      dataLines.push(line);
    }
  }
  if (dataLines.length < 2) return null;

  // Friendly run count: the `# runs:` header is "10 (hyperfine mean ...)" — the
  // website only wants the leading number so the caption reads "10 runs".
  const runsMatch = (meta.runs || "").match(/^\d+/);
  meta.runsCount = runsMatch ? runsMatch[0] : null;
  meta.toolVersions = parseToolVersions(meta.tools);

  // Column layout: `fixture`, one `<tool>_ms` per timed tool, then one
  // `<tool>_diags` per base tool (how many diagnostics the tool reported on
  // that fixture — the harness preflight writes it so a do-nothing run is
  // visible next to its time). A blank `_ms` cell means the tool FAILED to
  // analyze that fixture (exit >= 2) and the harness excluded it from timing.
  const msIdx = new Map();
  const diagIdx = new Map();
  dataLines[0].split(",").forEach((c, i) => {
    if (c.endsWith("_ms")) msIdx.set(c.slice(0, -"_ms".length), i);
    else if (c.endsWith("_diags")) diagIdx.set(c.slice(0, -"_diags".length), i);
  });
  const allTools = [...msIdx.keys()];
  // Cold comparison only: warm-cache variants (…-warm) aren't separate
  // checkers; their numbers stay in `values` for computeWarm.
  const tools = allTools.filter((t) => !t.endsWith("-warm"));
  const rows = dataLines.slice(1).map((line) => {
    const parts = line.split(",");
    const num = (i) =>
      i == null || parts[i] === undefined || parts[i] === "" ? null : parseFloat(parts[i]);
    const values = {};
    for (const [t, i] of msIdx) values[t] = num(i);
    const diags = {};
    for (const [t, i] of diagIdx) diags[t] = num(i);
    return { fixture: parts[0], name: fixtureName(parts[0]), values, diags };
  });
  return { meta, tools, allTools, rows };
}

function median(nums) {
  const s = [...nums].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

// Per-checker median cold full-file time, for the "how it compares" speed row.
// Every checker's own median over the fixtures it reported, so the comparison is
// each tool against the same bench harness. Warm/cache variants are excluded —
// this is the cold, from-scratch number. Self-measured, reproducible with
// `make bench`; never hand-typed, so the table can't drift from the CSV.
function computeToolMedians(rows, tools) {
  const ms = {};
  const text = {};
  for (const tool of tools) {
    if (tool.endsWith("-warm")) continue;
    const vals = rows.map((r) => r.values[tool]).filter((v) => v != null && v > 0);
    ms[tool] = vals.length ? Math.round(median(vals)) : null;
    text[tool] = ms[tool] == null ? "—" : `${ms[tool]} ms`;
  }
  const ranked = Object.entries(ms).filter(([, v]) => v != null);
  const fastest = ranked.length
    ? ranked.reduce((best, entry) => (entry[1] < best[1] ? entry : best))[0]
    : null;
  return { ms, text, fastest };
}

// Headline "how much faster than Pyright" stats for the benchmarks docs page.
// Computed from the SAME parsed CSV the table renders, so the punchy numbers can
// never drift from the measured data. Cold-vs-cold (both tools measured cold);
// self-measured, reproducible with `make bench` — never a hand-typed figure.
function computeVsPyright(rows) {
  const pairs = rows
    .map((r) => ({ b: r.values.basilisk, p: r.values.pyright }))
    .filter((x) => x.b != null && x.p != null && x.b > 0 && x.p > 0);
  if (!pairs.length) return null;
  const factors = pairs.map((x) => x.p / x.b);
  return {
    maxFactor: Math.round(Math.max(...factors)),
    medianFactor: Math.round(median(factors)),
    basiliskMedianMs: Math.round(median(pairs.map((x) => x.b))),
    pyrightMedianMs: Math.round(median(pairs.map((x) => x.p))),
    beats: pairs.filter((x) => x.b < x.p).length,
    total: pairs.length,
  };
}

// Per-tool distribution of the cold check across the fixture corpus. The
// published cold table is ONE ROW PER TOOL (median + fastest/slowest fixture),
// never a fixture × tool grid: for the heavier tools a cold single-file check
// is dominated by fixed startup + stub-loading cost, so a full grid would
// repeat each tool's baseline once per fixture and imply per-construct
// precision that doesn't exist (computeOutliers surfaces the departures that
// ARE real). `zeroDiag` counts fixtures where the tool ran but reported no
// diagnostics — published so a do-nothing run is visible next to its time.
// `missing` counts fixtures with no timing at all (tool not installed on that
// machine, or excluded by the harness preflight after failing to analyze).
function computeToolStats(rows, tools) {
  const stats = tools
    .map((tool) => {
      const measured = rows
        .map((r) => ({
          ms: r.values[tool],
          fixture: r.fixture,
          name: r.name,
          diags: r.diags[tool] ?? null,
        }))
        .filter((e) => e.ms != null && e.ms > 0);
      if (!measured.length) return null;
      const byMs = [...measured].sort((a, b) => a.ms - b.ms);
      const [min, max] = [byMs[0], byMs[byMs.length - 1]];
      const med = Math.round(median(measured.map((e) => e.ms)));
      const withDiags = measured.filter((e) => e.diags != null);
      return {
        tool,
        medianMs: med,
        medianText: `${med} ms`,
        min: { fixture: min.fixture, name: min.name, text: `${Math.round(min.ms)} ms` },
        max: { fixture: max.fixture, name: max.name, text: `${Math.round(max.ms)} ms` },
        measured: measured.length,
        missing: rows.length - measured.length,
        diagCounted: withDiags.length,
        zeroDiag: withDiags.filter((e) => e.diags === 0).length,
      };
    })
    .filter(Boolean)
    .sort((a, b) => a.medianMs - b.medianMs);
  return stats.map((s, i) => ({ ...s, fastest: i === 0 }));
}

// The per-construct signal that IS real: fixtures where a tool departs from
// its own median by at least 2× (failed-import resolution, dataclass
// synthesis, …). Below that threshold a per-fixture difference is within the
// noise of the tool's fixed startup cost, so it is never published per-fixture.
function computeOutliers(rows, tools) {
  const out = [];
  for (const tool of tools) {
    const vals = rows.map((r) => r.values[tool]).filter((v) => v != null && v > 0);
    if (vals.length < 3) continue;
    const med = median(vals);
    if (med <= 0) continue;
    for (const r of rows) {
      const ms = r.values[tool];
      if (ms != null && ms >= med * 2) {
        out.push({
          tool,
          fixture: r.fixture,
          name: r.name,
          ms: Math.round(ms),
          medianMs: Math.round(med),
          factor: Math.round((ms / med) * 10) / 10,
        });
      }
    }
  }
  return out.sort((a, b) => b.factor - a.factor);
}

// Warm re-check comparison, collapsed to a per-tool median (same
// one-row-per-tool shape as the cold table). Only two tools have a measured
// warm number: basilisk (`--cache`) and mypy (incremental `.mypy_cache`) —
// the `<tool>-warm` CSV columns. Pyright, ty and Pyrefly keep no cross-run
// result cache (empirically: no cache artifacts, no cache flag, a repeat run
// is just a warm-binary cold run); zuban's mypy mode DOES reuse a
// `.mypy_cache`, but the harness wipes it and measures zuban cold-only. All
// four are flagged `cached: false` and show their cold median — never a
// fabricated warm figure. Reuses the already-parsed `values`, so this table
// can never drift from the CSV.
function computeWarm(rows, allTools) {
  const warmSet = new Set(allTools.filter((t) => t.endsWith("-warm")));
  const baseTools = allTools.filter((t) => !t.endsWith("-warm"));
  // For each base tool, prefer its `-warm` column when one exists.
  const columns = baseTools
    .map((tool) => ({
      tool,
      key: warmSet.has(`${tool}-warm`) ? `${tool}-warm` : tool,
      cached: warmSet.has(`${tool}-warm`),
    }))
    .filter((c) => rows.some((r) => r.values[c.key] != null));
  // Only worth a table if at least one column is a genuine warm cache.
  if (!columns.some((c) => c.cached)) return { hasData: false, columns: [] };

  const stats = columns
    .map((c) => {
      const vals = rows.map((r) => r.values[c.key]).filter((v) => v != null && v > 0);
      const med = vals.length ? Math.round(median(vals)) : null;
      return { tool: c.tool, cached: c.cached, medianMs: med };
    })
    .filter((c) => c.medianMs != null)
    .sort((a, b) => a.medianMs - b.medianMs);
  return {
    hasData: stats.length > 0,
    columns: stats.map((c, i) => ({ ...c, text: `${c.medianMs} ms`, fastest: i === 0 })),
  };
}

// How many tool columns in a CSV carry at least one real measurement. A machine
// that only ran basilisk scores 1; a full competitor sweep scores every tool.
// Used to keep an incomplete CSV from ever becoming the site's primary and
// rendering a benchmark table full of empty competitor columns.
function toolCoverage(file) {
  const parsed = parseCsv(readFileSync(join(STATUS_DIR, file), "utf-8"));
  if (!parsed) return -1;
  return parsed.tools.filter((t) => parsed.rows.some((r) => r.values[t] != null))
    .length;
}

function pickPrimary(files) {
  // Explicit overrides win, in order: env var, then a committed .primary pin.
  const env = process.env.BASILISK_BENCH_PRIMARY;
  if (env && files.includes(`${env}.csv`)) return `${env}.csv`;
  const primaryFile = join(STATUS_DIR, ".primary");
  if (existsSync(primaryFile)) {
    const slug = readFileSync(primaryFile, "utf-8").trim();
    if (files.includes(`${slug}.csv`)) return `${slug}.csv`;
  }
  // Automatic fallback: NEVER let an incomplete CSV (e.g. a machine that only
  // ran basilisk) win and drop competitor columns. Rank by tool coverage first,
  // then stable CI hardware (gha-*), then alphabetical for determinism.
  const coverage = new Map(files.map((f) => [f, toolCoverage(f)]));
  return [...files].sort(
    (a, b) =>
      coverage.get(b) - coverage.get(a) ||
      (a.startsWith("gha-") ? 0 : 1) - (b.startsWith("gha-") ? 0 : 1) ||
      a.localeCompare(b),
  )[0];
}

export default function () {
  const empty = { available: [], primary: null, meta: {}, tools: [], rows: [], hasData: false };
  if (!existsSync(STATUS_DIR)) return empty;

  const files = readdirSync(STATUS_DIR).filter((f) => f.endsWith(".csv")).sort();
  if (files.length === 0) return empty;

  const primary = pickPrimary(files);
  const parsed = parseCsv(readFileSync(join(STATUS_DIR, primary), "utf-8"));
  if (!parsed) return empty;

  return {
    available: files.map((f) => f.replace(/\.csv$/, "")),
    primary: primary.replace(/\.csv$/, ""),
    ...parsed,
    toolMedians: computeToolMedians(parsed.rows, parsed.tools),
    toolStats: computeToolStats(parsed.rows, parsed.tools),
    outliers: computeOutliers(parsed.rows, parsed.tools),
    vsPyright: computeVsPyright(parsed.rows),
    warm: computeWarm(parsed.rows, parsed.allTools),
    hasData: parsed.rows.length > 0,
  };
}
