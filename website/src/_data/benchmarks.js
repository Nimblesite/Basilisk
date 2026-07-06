// Eleventy global data: benchmark results, read from the git-tracked per-machine
// CSV that `make bench` generates (benchmarks/status/<machine>.csv).
//
// The homepage benchmark table renders from this data, so the published numbers
// are always whatever was last measured + committed — never hand-typed.
//
// Primary machine selection (what the website shows):
//   1. $BASILISK_BENCH_PRIMARY (slug)         2. benchmarks/status/.primary file
//   3. first `gha-*` file (stable CI hardware) 4. first CSV alphabetically
import { readFileSync, readdirSync, existsSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const STATUS_DIR = join(__dirname, "../../../benchmarks/status");

const titleCase = (s) => s.charAt(0).toUpperCase() + s.slice(1);

// "e0002_missing_return" -> { code: "E0002", name: "Missing return" }
// `code` is language-neutral; `name` is the English fallback. The benchmark
// table macro is shared across locales — non-English pages pass a translated
// name map keyed by `fixture`, falling back to this English `name`.
function codeAndName(stem) {
  const [code, ...rest] = stem.split("_");
  const name = rest.join(" ").replace(/\b\w/, (c) => c.toUpperCase());
  return { code: code.toUpperCase(), name: name.trim() };
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

  const cols = dataLines[0].split(",");
  const allTools = cols.slice(1).map((c) => c.replace(/_ms$/, ""));
  // The per-rule table is a COLD-check comparison of type checkers, so the
  // warm-cache variants (…-warm) are excluded from `tools` (and thus the table
  // columns and the fastest mark) — they aren't separate checkers and would
  // crowd the real ones off the page. Their raw numbers stay in `values` (and
  // the committed CSV); only the rendered comparison drops them.
  const tools = allTools.filter((t) => !t.endsWith("-warm"));
  const rows = dataLines.slice(1).map((line) => {
    const parts = line.split(",");
    const stem = parts[0];
    const values = {};
    allTools.forEach((t, i) => {
      const v = parts[i + 1];
      values[t] = v === undefined || v === "" ? null : parseFloat(v);
    });
    const present = tools.filter((t) => values[t] != null);
    const fastest = present.reduce(
      (best, t) => (best == null || values[t] < values[best] ? t : best),
      null,
    );
    const { code, name } = codeAndName(stem);
    return {
      fixture: stem,
      code,
      name,
      label: `${code} ${name}`.trim(),
      values,
      fastest,
      cells: tools.map((t) => ({
        tool: t,
        ms: values[t],
        text: values[t] == null ? "—" : `${Math.round(values[t])} ms`,
        fastest: t === fastest,
      })),
    };
  });
  return { meta, tools, rows };
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
    vsPyright: computeVsPyright(parsed.rows),
    hasData: parsed.rows.length > 0,
  };
}
