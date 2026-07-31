// Eleventy global data: benchmark results, read from the git-tracked per-machine
// CSV that `make bench` generates (benchmarks/status/<machine>.csv).
//
// The website renders MEASURED FACTS. This loader parses the CSV's header and
// per-file timings for the benchmark page. It also derives each tool's median
// fresh-process check for the home pages. There are no speedup ratios, "beats M
// of N" tallies, or arbitrary outlier thresholds: published values are either
// CSV measurements or direct medians of those measurements.
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

// Parse the CSV `# tools:` header into [{ tool, version }] for the methodology
// footnote. The header looks like:
//   "basilisk=basilisk 0.0.0, pyright=pyright 1.1.408, mypy=mypy 1.19.1 (compiled: yes), ..."
// i.e. comma-separated `name=<--version output>` entries. The version output
// usually repeats the tool name and may carry a trailing parenthetical, both of
// which we strip so the site shows a clean "pyright 1.1.408". This is metadata
// pass-through — the harness records each installed tool's version output, and
// the page shows a cleaned form of that recorded value.
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
      // Dev builds don't have a released version number. Preserve the source
      // identifier emitted in 0.0.0-dev+g<sha> and keep a dirty marker visible.
      // A bare placeholder degrades to a plain "dev build" label.
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

  // Friendly minimum run count: the header begins with the number of Hyperfine
  // measurements required for every file, followed by the noisy-run policy.
  const runsMatch = (meta.runs || "").match(/^\d+/);
  meta.runsCount = runsMatch ? runsMatch[0] : null;
  meta.toolVersions = parseToolVersions(meta.tools);

  // Column layout: `fixture`, then one `<tool>_ms` per timed tool. Diagnostic
  // columns follow, but the benchmark page intentionally presents timings only.
  // A blank `_ms` cell means the tool was unavailable or failed preflight.
  const msIdx = new Map();
  dataLines[0].split(",").forEach((c, i) => {
    if (c.endsWith("_ms")) msIdx.set(c.slice(0, -"_ms".length), i);
  });
  const allTools = [...msIdx.keys()];
  // Warm-cache variants (…-warm) aren't separate checkers, so exclude them from
  // the cold medians used on the home pages. Their per-file values stay in rows.
  const tools = allTools.filter((t) => !t.endsWith("-warm"));
  const rows = dataLines.slice(1).map((line) => {
    const parts = line.split(",");
    const num = (i) =>
      i == null || parts[i] === undefined || parts[i] === "" ? null : parseFloat(parts[i]);
    const values = {};
    const valueText = {};
    for (const [tool, index] of msIdx) {
      values[tool] = num(index);
      valueText[tool] = parts[index] ? `${parts[index]} ms` : "—";
    }
    return {
      fixture: parts[0],
      filename: `${parts[0]}.py`,
      values,
      valueText,
    };
  });
  return { meta, tools, allTools, rows };
}

function median(nums) {
  const s = [...nums].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

// Per-checker median cold full-file time, for the "how it compares" speed row.
// Every checker's own median over the fixtures it reported — a direct order
// statistic of the measured CSV values, NOT a comparison number: the page shows
// each tool's median next to the others and lets the reader compare, rather than
// asserting a build-time "N× faster" ratio. Warm/cache variants are excluded;
// this is the fresh-process measurement without a persistent result-cache.
// Self-measured and reproducible with `make bench`, so it cannot drift from the
// CSV.
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

  // Everything exposed is either a CSV value or a median of CSV values.
  return {
    available: files.map((f) => f.replace(/\.csv$/, "")),
    primary: primary.replace(/\.csv$/, ""),
    ...parsed,
    toolMedians: computeToolMedians(parsed.rows, parsed.tools),
    hasData: parsed.rows.length > 0,
  };
}
