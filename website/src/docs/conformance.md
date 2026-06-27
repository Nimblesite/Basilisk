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

The [Python typing specification](https://typing.python.org/en/latest/spec/) defines how the type system should behave — generics, protocols, `TypedDict`, overloads, and the rest. To keep it honest, the typing community maintains a **conformance test suite** beside it in the [`python/typing`](https://github.com/python/typing/tree/main/conformance) repository.

It works like this:

- Each spec chapter has one or more **test files** — ordinary Python modules that exercise a feature and mark, with `# E` comments, every line where a conforming type checker **must** report an error (and, with `# E[tag]` groups, where one of several related errors is acceptable).
- A small **scoring tool** runs a type checker over those files and diffs its output against the annotations. A file *passes* only if the diff is empty: every required error is reported, and nothing is reported on a line the suite does not mark.
- The maintainers run every checker through it and publish the [results table](https://github.com/python/typing/blob/main/conformance/results/results.html) — the live, authoritative source for how pyright, mypy, pyrefly, ty, and the others currently score.

We use that suite at the pinned commit [`{{ conformance.pinnedRef }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance). The same tool and files grade everyone, so the number is comparable across checkers and not something we can tune in our favour.

## How a file is scored

The entire algorithm is two functions in the suite's `main.py` — `get_expected_errors` (reads the `# E` annotations) and `diff_expected_errors` (diffs them against the checker's output). A file passes **iff** that diff is empty:

- the suite's rule (`upstream_main.py:185`): `"Fail" if errors_diff.strip() else "Pass"`

We count **every** diagnostic the checker emits — errors *and* warnings, **no codes excluded**. That's the strictest reading, and how pyright (the reference checker) is graded: one unexpected diagnostic — a false positive — fails the whole file, so our false-positive count matters as much as the pass count.

## How we run it without forking it

The suite's `main.py` is the maintainers' batch harness — it grades every known checker at once and has no way to invoke ours. So, exactly as it does for every checker (`PyrightTypeChecker`, `MypyTypeChecker`, …), we add a thin **adapter** and reuse the suite's own scoring instead of reimplementing it. Our [`score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py):

1. **Adapter** — runs `basilisk check --output json` and shapes the result into the `{line: [errors]}` dict the suite's functions expect (the one thing the suite can't do for us).
2. **Calculator** — imports `get_expected_errors` and `diff_expected_errors` from a committed, byte-identical copy of the suite's `main.py` and calls them unmodified (`score.py:287` mirrors the suite's own call at `upstream_main.py:175`). It contains no scoring logic of its own.
3. **Gate** — compares the result against `coverage-thresholds.json` and fails CI on any regression.

To keep the calculator trustworthy, the vendored copy is **sha256-pinned**. `score.py` re-hashes it on every run and refuses to score if it has drifted (`score.py:99`), and this website re-hashes it again at build time:

{% if conformance.verified %}
<p><span class="conf-verified">✓ verified at build — conformance/upstream_main.py is {{ conformance.upstreamBytes }} bytes, sha256 {{ conformance.sha256Short }}…, matching the pin</span></p>
{% endif %}

The adapter and gate live in a separate, auditable file, so the calculator stays byte-for-byte the suite's own.

## What the score measures — and what it never runs

We score the binary exactly as a real user runs it, in its **default configuration**. Basilisk decides which rules run **purely from config**, and the default config matches the **core PEP conformance set, exactly** — nothing more. Before scoring, `score.py` *deletes* any `basilisk.json` from the fixtures directory, so a config file can neither silence a conformance rule nor quietly switch extra ones on. Disabling a conformance rule to lift the number is forbidden — and so is *deleting* its source file or unregistering it from the checker, the same dishonesty by another route.

Basilisk also ships **opt-in Basilisk rules** — extra checks the spec doesn't define, such as *require an annotation* on every parameter, return, and `*args`/`**kwargs`, a redundant-annotation warning, a missing-`@override` nudge, and an explicit-`Any` nudge. They turn on **only when you enable them in config**; a fresh install runs none of them. They are **not** conformance rules, the conformance run never executes them, and they have never added — or cost — a single point.

If anything, switching them on **breaks** PEP conformance. The spec treats an unannotated value as *inferred*, not an error — so a rule like *require an annotation* fires on perfectly spec-valid code and registers as a **false positive** against the suite. That is precisely why these rules ship off, and why conformance is measured against the default config: the plain binary, the core PEP set, and nothing else. Enable the extra rules in your own project when you want checking *stricter than the spec* — just know that "stricter than the spec" and "100% conformant to the spec" are different goals, and this score only ever measures the second.

## How the score changed

The site has shown a conformance number for a while, and it wasn't always measured honestly — we'd rather say so plainly than quietly paper over it. An earlier in-repo script inflated the figure by **excluding some diagnostic codes from the diff and not counting false positives at all**, so files that should have failed were scored as passing. We threw it out and adopted the official `python/typing` calculator, run unmodified on the real default binary.

That official figure is what you see today: **{{ conformance.scorePct }}%** ({{ conformance.pass }} / {{ conformance.total }} files, {{ conformance.fp }} false positives, {{ conformance.missed }} missed required errors), every conformance rule enabled and the opt-in Basilisk rules left exactly where a fresh install leaves them — off. The number is what the spec gives the out-of-the-box binary, nothing tuned in our favour.

The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time: one point per commit that changed it, plotting the score that commit actually recorded — including the correction when we switched from the in-repo script to the official calculator.

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
