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

// "e0002_missing_return" -> "E0002 Missing return"
function labelFor(stem) {
  const [code, ...rest] = stem.split("_");
  const human = rest.join(" ").replace(/\b\w/, (c) => c.toUpperCase());
  return `${code.toUpperCase()} ${human}`.trim();
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

  const cols = dataLines[0].split(",");
  const tools = cols.slice(1).map((c) => c.replace(/_ms$/, ""));
  const rows = dataLines.slice(1).map((line) => {
    const parts = line.split(",");
    const stem = parts[0];
    const values = {};
    tools.forEach((t, i) => {
      const v = parts[i + 1];
      values[t] = v === undefined || v === "" ? null : parseFloat(v);
    });
    const present = tools.filter((t) => values[t] != null);
    const fastest = present.reduce(
      (best, t) => (best == null || values[t] < values[best] ? t : best),
      null,
    );
    return {
      fixture: stem,
      label: labelFor(stem),
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

function pickPrimary(files) {
  const env = process.env.BASILISK_BENCH_PRIMARY;
  if (env && files.includes(`${env}.csv`)) return `${env}.csv`;
  const primaryFile = join(STATUS_DIR, ".primary");
  if (existsSync(primaryFile)) {
    const slug = readFileSync(primaryFile, "utf-8").trim();
    if (files.includes(`${slug}.csv`)) return `${slug}.csv`;
  }
  const gha = files.find((f) => f.startsWith("gha-"));
  return gha || files[0];
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
    hasData: parsed.rows.length > 0,
  };
}
