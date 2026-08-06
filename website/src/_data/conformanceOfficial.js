// Historical python/typing leaderboard snapshot retained only for the public
// record of Basilisk's withdrawn announcement. Basilisk is no longer listed in
// the live official results, and its row below is invalid as evidence of actual
// conformance because the implementation was fitted to exact fixtures.
//
//   _data/conformance.js  -> historical outputs from Basilisk's withdrawn run.
//   _data/conformanceOfficial.js (this file)
//                         -> the dated snapshot used in the retracted post.
//
// The snapshot is pinned so the retraction can show exactly what was published.
// It must never be described as current. The live source is linked separately.
//
// Source of every value below:
//   https://github.com/python/typing/blob/main/conformance/results/results.html
// as published in python/typing@3410759355c3018063d3a446102f88621fc43eb5,
// 2026-07-31. PR #2316 originally added Basilisk to the board. This snapshot is
// intentionally frozen; do not refresh it from the live leaderboard.

const SNAPSHOT = {
  source: "https://github.com/python/typing/blob/main/conformance/results/results.html",
  resultsDir: "https://github.com/python/typing/tree/3410759355c3018063d3a446102f88621fc43eb5/conformance/results",
  snapshotUrl: "https://github.com/python/typing/blob/3410759355c3018063d3a446102f88621fc43eb5/conformance/results/results.html",
  commitUrl: "https://github.com/python/typing/commit/3410759355c3018063d3a446102f88621fc43eb5",
  addedPrUrl: "https://github.com/python/typing/pull/2316",
  sha: "3410759",
  date: "2026-07-31",
  dateLabel: "Jul 31, 2026",
};

// Historical leaderboard grand-total row, verbatim from that snapshot.
// Basilisk's row and comparisons derived from it are withdrawn.
const TOOLS = [
  { id: "basilisk", name: "Basilisk", version: "0.27.0", org: null, pass: 141, total: 141 },
  { id: "pyright", name: "Pyright", version: "1.1.410", org: "Microsoft", pass: 136.5, total: 141 },
  { id: "mypy", name: "mypy", version: "2.1.0", org: null, pass: 109, total: 141 },
  { id: "ty", name: "ty", version: "0.0.65", org: "Astral", pass: 122, total: 141 },
  { id: "pyrefly", name: "Pyrefly", version: "1.1.0", org: "Meta", pass: 138, total: 141 },
  { id: "zuban", name: "zuban", version: "0.8.2", org: null, pass: 140.5, total: 141 },
  { id: "pycroscope", name: "pycroscope", version: "0.4.0", org: null, pass: 130, total: 141 },
];

const round1 = (n) => Math.round(n * 10) / 10;

export default function () {
  const enrich = (t) => ({
    ...t,
    pct: round1((t.pass / t.total) * 100),
    // A whole-number pass renders as "141"; a half-point as "140.5".
    passLabel: Number.isInteger(t.pass) ? String(t.pass) : t.pass.toFixed(1),
    resultsUrl: `${SNAPSHOT.resultsDir}/${t.id}`,
  });

  const tools = TOOLS.map(enrich);
  const byId = Object.fromEntries(tools.map((t) => [t.id, t]));
  const ranked = [...tools]
    .sort((a, b) => b.pct - a.pct)
    .map((t, i) => ({ ...t, rank: i + 1 }));

  const basilisk = byId.basilisk;
  const perfect = tools.filter((t) => t.pass === t.total);

  return {
    hasData: true,
    withdrawn: true,
    publicationStatus: "historical-withdrawn",
    historical: {
      snapshot: SNAPSHOT,
      tools,
      byId,
      ranked,
      basilisk,
      basiliskRankAtSnapshot: ranked.find((t) => t.id === "basilisk").rank,
      perfectCountAtSnapshot: perfect.length,
      basiliskWasSolePerfectAtSnapshot:
        perfect.length === 1 && perfect[0].id === "basilisk",
    },
  };
}
