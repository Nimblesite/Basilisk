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

Today that gives **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, {{ conformance.caught }} required errors caught, with **{{ conformance.fp }} false positives** and **{{ conformance.missed }} missed required errors** left to clear. {{ conformance.categoriesPass100 }} of {{ conformance.categoriesTotal }} categories pass at 100%. The target is 100%; we ratchet toward it.

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
- The maintainers run every checker through it and publish the [results table](https://github.com/python/typing/blob/main/conformance/results/results.html), which is how figures like pyright's ~99% or pyrefly's ~86% are produced.

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

## Every rule runs — there is no "conformance mode"

We score the binary exactly as a real user runs it: **every rule enabled and present, no configuration, nothing disabled, nothing deleted.** Before scoring, `score.py` *deletes* any `basilisk.json` from the fixtures directory so config cannot quietly silence a rule. Disabling a rule to lift the number is forbidden — and so is *deleting* the rule's source file or unregistering it from the checker, which is the same dishonesty by another route.

That honesty costs us points, on purpose. Basilisk is strict by default and layers on house-style rules the typing spec doesn't define — chiefly *require an annotation* on every parameter, return, and `*args`/`**kwargs`, plus a redundant-annotation warning, a missing-`@override` nudge, and an explicit-`Any` nudge. The spec treats an unannotated type as **inferred**, not an error, so these rules fire across the suite and count as false positives. They are the bulk of today's **{{ conformance.fp }}** false positives.

We could make the number look better tomorrow by turning those rules off at score time. We won't. The published figure has to mean *what you actually get out of the box*. The only legitimate way to 100% is to make the checker smarter — so its strict defaults stop firing on spec-valid code — with every rule still switched on. (You're free to relax any rule **in your own project**; the conformance scorer never does.)

## How the score changed

The site has shown a conformance number for a while; it hasn't always been honest, and we'd rather say so plainly than quietly fix it. Two different measurement shortcuts once inflated it. First, an in-repo script that excluded some diagnostic codes and didn't count false positives. We replaced that with the official calculator — but then a second shortcut crept in: the scorer ran the binary in a "spec-conformance mode" that **disabled six strict-by-default rules** before scoring, which pushed the reported number to a **fake 100%**. That is exactly the kind of gaming this page exists to prevent.

Both shortcuts are gone. We now run the official calculator over the binary with **every rule enabled** — the real out-of-the-box experience — and the honest figure today is **{{ conformance.scorePct }}%** ({{ conformance.pass }} / {{ conformance.total }} files, {{ conformance.fp }} false positives, {{ conformance.missed }} missed required errors). The number dropped not because the checker got worse, but because we stopped measuring it dishonestly. 100% is the target we ratchet toward — by fixing the checker, never by switching a rule off.

The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time: one point per commit that changed it, plotting the score that commit actually recorded — including the drop from the gamed 100% to the honest figure.

{{ chart(conformance, {
  "label": "Conformance score over time",
  "heading": "From a rules-disabled 100% to the honest, every-rule-enabled figure",
  "prevLegend": "Measured with rules disabled or codes excluded (not the real out-of-box behaviour)",
  "officialLegend": "Official <code>python/typing</code> calculator, every rule enabled",
  "dropNote": "Earlier runs reported up to <strong>" + conformance.chart.peak.score + "%</strong> — but only by disabling rules or excluding codes. Scored honestly with every rule enabled, the real figure is <strong>" + conformance.chart.current.score + "%</strong>. The drop is a correction, not a regression: the checker never got worse, the measurement got honest.",
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
