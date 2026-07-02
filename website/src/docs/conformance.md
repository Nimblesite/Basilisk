---
layout: layouts/docs.njk
title: "How Basilisk Measures PEP Conformance"
description: "How Basilisk's PEP conformance score is measured with the official python/typing conformance suite — what the suite is, how scoring works, the byte-identical pinned calculator we run, and why we score with every rule enabled and never disable one."
keywords: pep conformance, python typing conformance suite, basilisk conformance score, type checker scoring, python/typing calculator
date: 2026-06-23
dateModified: 2026-06-24
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 10
---
{% from "conformance-chart.njk" import chart %}

# How we measure PEP conformance

Basilisk is scored by the **official `python/typing` conformance suite** — the same test suite and scoring tool the typing community uses to grade pyright, mypy, pyrefly, ty, and others. We run that tool unmodified, on the real `basilisk` binary, on every change.

Today that gives **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, {{ conformance.caught }} required errors caught, with **{{ conformance.fp }} false positives** and **{{ conformance.missed }} missed required errors**. {{ conformance.categoriesPass100 }} of {{ conformance.categoriesTotal }} categories pass at 100%, and the ratchet gate keeps it from regressing.

<p class="conf-links">
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance suite &amp; README ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener">Published results ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py" target="_blank" rel="noopener">Our scorer — score.py ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py" target="_blank" rel="noopener">Vendored calculator ↗</a>
</p>

## What the conformance suite is

The [Python typing specification](https://typing.python.org/en/latest/spec/) defines how the type system should behave — generics, protocols, `TypedDict`, overloads, and the rest. To keep it honest, the typing community maintains a **conformance test suite** beside it in the [`python/typing`](https://github.com/python/typing/tree/main/conformance) repository: ordinary Python modules that mark, with `# E` comments, every line where a conforming checker **must** report an error. A scoring tool diffs a checker's output against those annotations, and the maintainers publish the [results table](https://github.com/python/typing/blob/main/conformance/results/results.html) for every major checker.

We score against the exact commit of the suite we last pulled from `main` — [`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}, {{ conformance.commitDate }}{% endif %} — recorded by its full hash, so the link stays fixed at the exact files we graded.

That pin never goes stale: we run in **lock step** with `python/typing@main`. Every `make test`, every CI run of the checker, and a dedicated release job re-resolve the *current* tip, re-download the suite when it has moved, and re-grade the binary at **100% pass, 0 false positives** — an upstream test we fail blocks merge and release until the checker conforms. The commit is inserted into every page automatically from the scorer's report, never typed by hand.

## How a file is scored

The entire algorithm is two functions in the suite's `main.py` — `get_expected_errors` (reads the `# E` annotations) and `diff_expected_errors` (diffs them against the checker's output). A file passes **iff** that diff is empty (`upstream_main.py:185`: `"Fail" if errors_diff.strip() else "Pass"`): every required error reported, nothing reported on an unmarked line. We count **every** diagnostic the checker emits — errors *and* warnings, no codes excluded — so a single false positive fails the whole file.

## How we run it without forking it

The suite's `main.py` grades every known checker at once and has no way to invoke ours. So, exactly as the suite does for pyright and mypy, [`score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py) adds a thin **adapter** and reuses the suite's own scoring:

1. **Adapter** — runs `basilisk check --output json` and shapes the result into the `{line: [errors]}` dict the suite's functions expect.
2. **Calculator** — imports `get_expected_errors` and `diff_expected_errors` from a committed, byte-identical copy of the suite's `main.py` and calls them unmodified — no scoring logic of our own.
3. **Gate** — compares the result against `coverage-thresholds.json` and fails CI on any regression.

The vendored calculator is **sha256-pinned**: `score.py` re-hashes it on every run and refuses to score if it has drifted, and this website re-hashes it again at build time:

{% if conformance.verified %}
<p><span class="conf-verified">✓ verified at build — conformance/upstream_main.py is {{ conformance.upstreamBytes }} bytes, sha256 {{ conformance.sha256Short }}…, matching the pin</span></p>
{% endif %}

## What the score measures — and what it never runs

We score the binary exactly as a real user runs it: the **default configuration**, which enables the **core PEP conformance set** — nothing more. Before scoring, `score.py` *deletes* any `basilisk.json` from the fixtures directory, so a config file can neither silence a conformance rule nor quietly switch extra ones on. Disabling a conformance rule to lift the number is forbidden — and so is deleting or unregistering it.

Basilisk's **opt-in rules** (require-annotation, redundant-annotation, missing-`@override`, explicit-`Any`) never run during scoring; a fresh install runs none of them. Enabling them would *lower* the score, not raise it: the spec treats an unannotated value as *inferred*, not an error, so require-annotation fires on spec-valid code and counts as a **false positive**. "Stricter than the spec" and "conformant to the spec" are different goals — this score measures only the second.

## How the score changed

The score wasn't always measured honestly, and we'd rather say so plainly than paper over it. An earlier in-repo script inflated the figure by **excluding some diagnostic codes from the diff and not counting false positives at all**; we threw it out and adopted the official `python/typing` calculator, run unmodified on the real default binary. The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time — one point per commit that changed it, including that correction.

{{ chart(conformance, {
  "label": "Conformance score over time",
  "heading": "From an in-repo script to the official calculator",
  "prevLegend": "Earlier in-repo script — excluded codes, ignored false positives (not the official measure)",
  "officialLegend": "Official <code>python/typing</code> calculator on the real default binary",
  "dropNote": "Early points came from an in-repo script that excluded diagnostic codes and didn&rsquo;t count false positives; later points use the official <code>python/typing</code> calculator on the real default binary. Today&rsquo;s official figure is <strong>" + conformance.chart.current.score + "%</strong> — a measurement that got honest, not a checker that got worse.",
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
