---
layout: layouts/docs.njk
title: "How Basilisk Calculates PEP Conformance"
description: "Exactly how Basilisk's PEP conformance score is computed — the byte-identical, sha256-pinned python/typing calculator, the wrapper, the proof it isn't an approximation, and the honest correction from a rigged 100% to the real number."
keywords: pep conformance, python typing conformance suite, basilisk conformance score, type checker scoring, python/typing calculator
date: 2026-06-23
dateModified: 2026-06-23
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 8
---
{% from "conformance-chart.njk" import chart %}

# How we calculate PEP conformance

Basilisk's headline conformance number is **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, with **{{ conformance.fp }} false positives** and **{{ conformance.missed }} missed required errors** still to clear. {{ conformance.caught }} required errors are caught. {{ conformance.categoriesPass100 }} of {{ conformance.categoriesTotal }} categories pass at 100%.

We do not grade ourselves. The number above is produced by the **official `python/typing` conformance calculator** — the exact tool that grades pyright, mypy, pyrefly, ty, zuban, and pycroscope — run **unmodified**. This page shows precisely how, proves it is the real tool and not an approximation, and is fully transparent about the bug that once inflated this number to a fake 100%.

{% if conformance.verified %}
<p><span class="conf-verified">✓ verified at build — upstream_main.py sha256 {{ conformance.sha256Short }}… matches the pin</span></p>
{% endif %}

## Full transparency: the number used to be wrong

For months this site reported a number that climbed all the way to **100%**. That was a lie produced by a since-removed *in-repo* harness that **excluded 9 diagnostic codes from scoring and ignored false positives entirely**. When we replaced it with the real `python/typing` calculator, the honest number dropped to **{{ conformance.scorePct }}%**.

<div class="conf-correction">
  <span class="conf-correction__old">100%</span>
  <span class="conf-correction__arrow">&rarr;</span>
  <span class="conf-correction__new">{{ conformance.scorePct }}%</span>
  <span class="conf-correction__text">Not a regression — a correction. The checker did not get worse; the scorer got honest. 100% remains the <strong>target</strong>, not a present-day claim.</span>
</div>

The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time — one point per commit that changed the file, plotting the score that commit actually recorded. Nothing here is hand-typed.

{{ chart(conformance, {
  "label": "Conformance score over time",
  "heading": "A rigged climb to 100%, then the official calculator told the truth",
  "riggedLegend": "Old in-repo harness — excluded 9 codes, ignored false positives",
  "officialLegend": "Official <code>python/typing</code> calculator",
  "dropNote": "On <strong>" + conformance.chart.peak.shortDate + "</strong> the in-repo harness reported a full <strong>" + conformance.chart.peak.score + "%</strong>. Run for the first time on <strong>" + conformance.chart.current.shortDate + "</strong>, the official calculator reports <strong>" + conformance.chart.current.score + "%</strong>.",
  "caption": "Each dot is a real commit to <code>conformance/conformance_status.csv</code>; the series is its git log, recomputed every build. Hover a point for its date, commit, score, and false-positive count."
}) }}

## Proof the scoring is the official tool, not an approximation

Four checks, all reproducible against the files in this repository.

### (a) The calculator is byte-identical to upstream

Download `conformance/src/main.py` from [`python/typing@{{ conformance.pinnedRef }}`](https://github.com/python/typing/blob/main/conformance/src/main.py) and diff it against our committed [`conformance/upstream_main.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py):

- **upstream (downloaded):** `{{ conformance.sha256Short }}…`
- **committed in repo:** `{{ conformance.liveSha256Short }}…`

Same sha256, same {{ conformance.upstreamBytes }} bytes, **zero-line diff**. It is *the* file, not a copy-with-edits.

### (b) We call upstream's own functions — we don't reimplement them

The entire scoring algorithm is two functions, `get_expected_errors` and `diff_expected_errors`, inside that committed file. `score.py` imports them and calls them; it contains **zero scoring logic of its own**. The call shapes match upstream's own call in the same file:

- upstream (`upstream_main.py:175`): `diff_expected_errors(type_checker, test_case, output, ignored_errors)`
- ours (`score.py:287`): `diff_errors(checker, f, output, [])`

Same four arguments, same order.

### (c) Pass/fail is upstream's exact rule

A file passes **iff** the diff string is empty — upstream's literal rule:

- upstream (`upstream_main.py:185`): `"Fail" if errors_diff.strip() else "Pass"`
- ours (`score.py:291`): `passed = not diff.strip()`

### (d) Tamper-proofing is live

`score.py` re-hashes the calculator on **every run** and refuses to score if the sha256 doesn't match the pin (`score.py:99`), so the official file cannot silently drift. This website re-hashes it again at build time — that is the green badge above.

### (e) It runs and produces the number

Live, against the real compiled binary: **{{ conformance.scorePct }}% ({{ conformance.pass }}/{{ conformance.total }})**, {{ conformance.fp }} false positives, {{ conformance.missed }} missed — gate **PASS**. That is the strictest grading: **errors *and* warnings count**, the same way the reference checker pyright is graded upstream.

## Why a wrapper exists at all

The only Basilisk-specific code is a `BasiliskTypeChecker` **adapter** — and even that is not a departure from the method. Upstream requires one adapter per checker (`PyrightTypeChecker`, `MypyTypeChecker`, …); ours runs `basilisk check --output json` and shapes the result into the `{line: [errors]}` dict the official functions consume. That is the contract every checker fulfills.

`upstream_main.py` cannot be run directly to score Basilisk — **by design**. It is a batch test harness for the `python/typing` maintainers, not a single-checker scorer:

- it imports `tomli`, `tomlkit`, `options`, `reporting`, `test_groups`, `type_checker` at module load — extra deps and a TOML config/reporting pipeline irrelevant to "score this one binary";
- it has no Basilisk adapter — it only knows pyright/mypy/pyrefly/ty, with no way to invoke our binary;
- it writes per-checker TOML result files and an HTML matrix across all checkers — not a CI gate.

So the wrapper is the **minimum glue** to use upstream's real scoring without forking it:

1. **Adapter** — run the `basilisk` binary, turn its JSON into the `{line: errors}` dict (the one thing upstream genuinely can't do for us).
2. **Loader** — import the two scoring functions out of the committed file behind stubs for those unrelated imports, *after* verifying the sha256. The stub module is not manipulation of the scoring — the two functions touch none of those imports; it just lets the file import when `tomlkit` et al. aren't installed.
3. **Gate** — compare the live {{ conformance.scorePct }}% / {{ conformance.fp }} against `coverage-thresholds.json` and exit non-zero on any regression.

The alternative — editing `upstream_main.py` to add our adapter and strip its deps — would break the byte-identical guarantee that makes proof (a) possible. The wrapper exists precisely so the official file stays untouched and verifiable. **The split is the honest one: official calculator = committed and unmodified; our glue = a separate, auditable file.**

## Where each category stands today

Read live from `conformance/conformance_status.csv` at build time:

<table>
  <thead><tr><th>Category</th><th>Passing</th><th>Score</th><th></th></tr></thead>
  <tbody>
  {% for cat in conformance.categories %}
    <tr>
      <td>{{ cat.label }}</td>
      <td>{{ cat.pass }} / {{ cat.total }}</td>
      <td>{{ cat.pct }}%</td>
      <td><span class="conf-cat-bar" style="width: {{ (cat.pct * 1.2) | round }}px; opacity: {{ 0.35 + cat.pct / 154 }}"></span></td>
    </tr>
  {% endfor %}
  </tbody>
</table>

## Reproduce it yourself

```bash
# Builds the binary, fetches the (git-ignored) fixtures, runs the official
# python/typing calculator against them, writes conformance_status.csv, and
# enforces the ratchet gate from coverage-thresholds.json.
make conformance
```

Everything above lives in two files: [`conformance/score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py) (our auditable glue) and [`conformance/upstream_main.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py) (the official calculator, committed and sha256-pinned). The full annotation rules are documented in the [python/typing conformance README](https://github.com/python/typing/blob/main/conformance/README.md).
