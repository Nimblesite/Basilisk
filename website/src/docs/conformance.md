---
layout: layouts/docs.njk
title: "How Basilisk Measures PEP Conformance"
description: "How Basilisk's PEP conformance score is measured with the official python/typing conformance suite — what the suite is, how scoring works, the byte-identical pinned calculator we run, and the correction we made to our own scoring."
keywords: pep conformance, python typing conformance suite, basilisk conformance score, type checker scoring, python/typing calculator
date: 2026-06-23
dateModified: 2026-06-23
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 8
---
{% from "conformance-chart.njk" import chart %}

# How we measure PEP conformance

Basilisk is scored by the **official `python/typing` conformance suite** — the same test suite and scoring tool the typing community uses to grade pyright, mypy, pyrefly, ty, and others. We run that tool unmodified, on the real `basilisk` binary, on every change.

Today that gives **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, {{ conformance.caught }} required errors caught, with **{{ conformance.fp }} false positives** and **{{ conformance.missed }} missed required errors** left to clear. {{ conformance.categoriesPass100 }} of {{ conformance.categoriesTotal }} categories pass at 100%. The target is 100%; we ratchet toward it.

<p class="conf-links">
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance suite &amp; README ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener">Published results ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py" target="_blank" rel="noopener">Our scorer — score.py ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py" target="_blank" rel="noopener">Vendored calculator ↗</a>
</p>

## What the conformance suite is

The [Python typing specification](https://typing.python.org/en/latest/spec/) defines how the type system is supposed to behave — generics, protocols, dataclasses, `TypedDict`, overloads, literals, and the rest. To stop the spec from being aspirational, the typing community maintains a **conformance test suite** alongside it in the [`python/typing`](https://github.com/python/typing/tree/main/conformance) repository.

It works like this:

- Each spec chapter has one or more **test files** — ordinary Python modules that exercise a feature and mark, with `# E` comments, every line where a conforming type checker **must** report an error (and, with `# E[tag]` groups, where one of several related errors is acceptable).
- A small **scoring tool** runs a type checker over those files and diffs its output against the annotations. A file *passes* only if the diff is empty: every required error is reported, and nothing is reported on a line the suite does not mark.
- The maintainers run every checker through it and publish the [results table](https://github.com/python/typing/blob/main/conformance/results/results.html), which is how figures like pyright's ~99% or pyrefly's ~86% are produced.

This is the suite we use, at the pinned commit [`{{ conformance.pinnedRef }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance). Because the same tool and the same files grade everyone, the number is comparable across checkers and is not something we can tune in our favour.

## How a file is scored

The entire algorithm is two functions in the suite's `main.py` — `get_expected_errors` (reads the `# E` annotations) and `diff_expected_errors` (diffs them against the checker's output). A file passes **iff** that diff is empty:

- the suite's rule (`upstream_main.py:185`): `"Fail" if errors_diff.strip() else "Pass"`

We count **every** diagnostic the checker emits — errors *and* warnings, with **no diagnostic codes excluded**. That is the strictest reading of the suite and matches how the reference checker, pyright, is graded. One unexpected diagnostic (a false positive) fails the whole file, which is why our false-positive count matters as much as the pass count.

## How we run it without forking it

The suite's `main.py` is a batch harness for the `python/typing` maintainers: it grades all the known checkers at once, pulls in TOML config/reporting dependencies, and writes a results matrix. It has no way to invoke our binary. So, exactly as the suite does for every checker (`PyrightTypeChecker`, `MypyTypeChecker`, …), we add a thin **adapter** and reuse the suite's own scoring rather than reimplementing it. Our [`score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py):

1. **Adapter** — runs `basilisk check --output json` and shapes the result into the `{line: [errors]}` dict the suite's functions expect (the one thing the suite can't do for us).
2. **Calculator** — imports `get_expected_errors` and `diff_expected_errors` from a committed, byte-identical copy of the suite's `main.py` and calls them unmodified (`score.py:287` mirrors the suite's own call at `upstream_main.py:175`). It contains no scoring logic of its own.
3. **Gate** — compares the result against `coverage-thresholds.json` and fails CI on any regression.

To keep the calculator trustworthy, the vendored copy is **sha256-pinned**. `score.py` re-hashes it on every run and refuses to score if it has drifted (`score.py:99`), and this website re-hashes it again at build time:

{% if conformance.verified %}
<p><span class="conf-verified">✓ verified at build — conformance/upstream_main.py is {{ conformance.upstreamBytes }} bytes, sha256 {{ conformance.sha256Short }}…, matching the pin</span></p>
{% endif %}

Keeping the official file untouched is the whole point: the adapter and gate live in a separate, auditable file, so the calculator stays byte-for-byte the suite's own.

## A correction we made

Our score used to be measured by an in-repo script of our own, and it was **wrong**. That script excluded several diagnostic codes from scoring and did not count false positives, so it reported numbers that climbed all the way to 100%. It was an honest mistake, not a tuned result — but it was still incorrect.

We replaced it with the official calculator described above. With every diagnostic counted and nothing excluded, the honest number is **{{ conformance.scorePct }}%**:

<div class="conf-correction">
  <span class="conf-correction__old">100%</span>
  <span class="conf-correction__arrow">→</span>
  <span class="conf-correction__new">{{ conformance.scorePct }}%</span>
  <span class="conf-correction__text">The checker did not get worse — the measurement got correct. 100% is the target we are working toward, not a claim about today.</span>
</div>

The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time: one point per commit that changed it, plotting the score that commit actually recorded.

{{ chart(conformance, {
  "label": "Conformance score over time",
  "heading": "From the earlier in-repo number to the official calculator",
  "prevLegend": "Earlier in-repo script (some codes excluded, false positives not counted)",
  "officialLegend": "Official <code>python/typing</code> calculator",
  "dropNote": "On <strong>" + conformance.chart.peak.shortDate + "</strong> the in-repo script reported <strong>" + conformance.chart.peak.score + "%</strong>. The official calculator, first run on <strong>" + conformance.chart.current.shortDate + "</strong>, reports <strong>" + conformance.chart.current.score + "%</strong> — a correction, not a regression.",
  "caption": "Each dot is a real commit to <code>conformance/conformance_status.csv</code>, recomputed every build. Hover a point for its date, commit, score, and false-positive count."
}) }}

## Where each category stands today

Read live from `conformance/conformance_status.csv` at build time:

<div class="table-wrapper">
<table>
<thead><tr><th>Category</th><th>Passing</th><th>Score</th><th></th></tr></thead>
<tbody>
{%- for cat in conformance.categories %}
<tr><td>{{ cat.label }}</td><td>{{ cat.pass }} / {{ cat.total }}</td><td>{{ cat.pct }}%</td><td><span class="conf-cat-bar" style="width: {{ (cat.pct * 1.2) | round }}px; opacity: {{ 0.4 + cat.pct / 170 }}"></span></td></tr>
{%- endfor %}
</tbody>
</table>
</div>

## Reproduce it yourself

```bash
# Builds the binary, fetches the (git-ignored) fixtures, runs the official
# python/typing calculator against them, writes conformance_status.csv, and
# enforces the ratchet gate from coverage-thresholds.json.
make conformance
```

It all lives in two files: [`conformance/score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py) (our adapter + gate) and [`conformance/upstream_main.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py) (the suite's calculator, committed and sha256-pinned). The full annotation rules are in the [python/typing conformance README](https://github.com/python/typing/blob/main/conformance/README.md).
