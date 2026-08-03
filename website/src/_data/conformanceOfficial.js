// Eleventy global data: the OFFICIAL python/typing conformance results — the
// same single run that grades every listed type checker, Basilisk included.
// Implements [CHKARCH-CONFORMANCE]; complements _data/conformance.js.
//
//   _data/conformance.js  -> Basilisk's OWN, reproducible, per-release score
//                            (we re-run the unmodified scorer every ship).
//   _data/conformanceOfficial.js (this file)
//                         -> a dated, transcribed SNAPSHOT of the upstream
//                            results.html leaderboard, so the comparison table
//                            can show every tool graded on ONE identical run.
//
// Honesty contract (see CLAUDE.md "Documentation Honesty"): competitor scores
// drift as those tools improve, so this snapshot is (a) pinned to the exact
// upstream commit that produced it, (b) labelled with that date wherever it
// renders, and (c) every cell links to that tool's LIVE results folder so a
// reader can check the current figure. The numbers are transcribed verbatim
// from the leaderboard totals — never approximated — and `pct` is DERIVED from
// pass/total here so a typo can never desync the percentage from its fraction.
//
// Source of every value below:
//   https://github.com/python/typing/blob/main/conformance/results/results.html
// as published in python/typing@3410759355c3018063d3a446102f88621fc43eb5,
// 2026-07-31. PR #2316 originally added Basilisk to the board. Re-transcribe
// (and bump `snapshot`) when upstream re-runs the suite.

const SNAPSHOT = {
  source: "https://github.com/python/typing/blob/main/conformance/results/results.html",
  resultsDir: "https://github.com/python/typing/tree/main/conformance/results",
  snapshotUrl: "https://github.com/python/typing/blob/3410759355c3018063d3a446102f88621fc43eb5/conformance/results/results.html",
  commitUrl: "https://github.com/python/typing/commit/3410759355c3018063d3a446102f88621fc43eb5",
  addedPrUrl: "https://github.com/python/typing/pull/2316",
  sha: "3410759",
  date: "2026-07-31",
  dateLabel: "Jul 31 2026",
};

// Leaderboard grand-total row, verbatim from results.html. `org` names the
// backer for the honest "beat Meta/Microsoft/Astral" framing; null = independent.
// Half-points are the suite's own scoring for partially-conformant test files.
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
    snapshot: SNAPSHOT,
    tools,
    byId,
    ranked,
    basilisk,
    // Basilisk's standing on the board, computed — never asserted by hand.
    basiliskRank: ranked.find((t) => t.id === "basilisk").rank,
    perfectCount: perfect.length,
    basiliskIsSolePerfect: perfect.length === 1 && perfect[0].id === "basilisk",
  };
}
