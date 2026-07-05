---
layout: layouts/docs.njk
title: "How Basilisk Measures PEP Conformance"
description: "How Basilisk's PEP conformance score is measured with the official python/typing conformance suite — what the suite is, how scoring works, how the wheel-installed CLI is submitted upstream, and why we score with every rule enabled and never disable one."
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

Basilisk is scored by the **official `python/typing` conformance suite** — the same test suite and scoring tool the typing community uses to grade pyright, mypy, pyrefly, ty, and others. Release proof runs that tool unmodified through a wheel-installed `basilisk` command, the same entry point users get from PyPI.

Today that gives **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, {{ conformance.caught }} required errors caught, with **{{ conformance.fp }} false positives** and **{{ conformance.missed }} missed required errors**. {{ conformance.categoriesPass100 }} of {{ conformance.categoriesTotal }} categories pass at 100%, and the ratchet gate keeps it from regressing.

<p class="conf-links">
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance suite &amp; README ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener">Published results ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py" target="_blank" rel="noopener">Submission script ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/typing-conformance-pr.md" target="_blank" rel="noopener">PR workflow ↗</a>
</p>

## What the conformance suite is

The [Python typing specification](https://typing.python.org/en/latest/spec/) defines how the type system should behave — generics, protocols, `TypedDict`, overloads, and the rest. To keep it honest, the typing community maintains a **conformance test suite** beside it in the [`python/typing`](https://github.com/python/typing/tree/main/conformance) repository: ordinary Python modules that mark, with `# E` comments, every line where a conforming checker **must** report an error. A scoring tool diffs a checker's output against those annotations, and the maintainers publish the [results table](https://github.com/python/typing/blob/main/conformance/results/results.html) for every major checker.

We score against the exact commit of the suite we last pulled from `main` — [`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}, {{ conformance.commitDate }}{% endif %} — recorded by its full hash, so the link stays fixed at the exact files we graded.

That pin never goes stale: we run in **lock step** with `python/typing@main`. Every `make test`, every CI run of the checker, and a dedicated release job re-resolve the *current* tip, re-download the suite when it has moved, and re-grade the binary at **100% pass, 0 false positives** — an upstream test we fail blocks merge and release until the checker conforms. The commit is inserted into every page automatically from the scorer's report, never typed by hand.

## How a file is scored

The official harness reads the suite's `# E` annotations, runs the checker adapter, and diffs expected diagnostics against observed diagnostics. A file passes only when that diff is empty: every required error reported, nothing reported on an unmarked line. We count **every** diagnostic the checker emits — errors *and* warnings, no codes excluded — so a single false positive fails the whole file.

## How we run it without forking it

The upstream suite grades only the checkers registered in its own `type_checker.py`. For release and submission proof, [`scripts/prepare_typing_conformance_pr.py`](https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py) patches a fresh `python/typing` checkout exactly the way an upstream PR needs it:

1. **Adapter** — injects `BasiliskTypeChecker`, which runs `basilisk check . --output json --color never` and passes all nonblank diagnostics to the upstream parser.
2. **Wheel install** — adds the `basilisk-python` dependency, refreshes `uv.lock`, and verifies that `basilisk --version` resolves from the `python/typing` virtual environment.
3. **Harness** — runs `uv run --python 3.12 python src/main.py --only-run basilisk`, then `--report-only`, producing the upstream `results/basilisk/*.toml` files and `results.html`.

That is the path used for submission. The old Basilisk-local scorer is only a development shortcut; it is not the source of truth for publishing or for an upstream conformance PR.

## What the score measures — and what it never runs

We score the CLI exactly as a real user runs it from the wheel: the **default configuration**, which enables the **core PEP conformance set** — nothing more. The release/submission gate runs in a clean `python/typing` checkout through the wheel-installed `basilisk` command, so no repo-local config can silence a conformance rule or quietly switch extra rules on. Disabling a conformance rule to lift the number is forbidden — and so is deleting or unregistering it.

Basilisk's **opt-in rules** (require-annotation, redundant-annotation, missing-`@override`, explicit-`Any`) never run during scoring; a fresh install runs none of them. Enabling them would *lower* the score, not raise it: the spec treats an unannotated value as *inferred*, not an error, so require-annotation fires on spec-valid code and counts as a **false positive**. "Stricter than the spec" and "conformant to the spec" are different goals — this score measures only the second.

## How the score changed

The score wasn't always measured honestly, and we'd rather say so plainly than paper over it. An earlier in-repo script inflated the figure by **excluding some diagnostic codes from the diff and not counting false positives at all**; we threw it out and adopted the official `python/typing` scoring semantics, run on the real default CLI. The chart below is read straight from the **git history of `conformance/conformance_status.csv`** at build time — one point per commit that changed it, including that correction.

{{ chart(conformance, {
  "label": "Conformance score over time",
  "heading": "From an in-repo script to the official harness",
  "prevLegend": "Earlier in-repo script — excluded codes, ignored false positives (not the official measure)",
  "officialLegend": "Official <code>python/typing</code> harness on the real default CLI",
  "dropNote": "Early points came from an in-repo script that excluded diagnostic codes and didn&rsquo;t count false positives; later points use the official <code>python/typing</code> scoring semantics on the real default CLI. Today&rsquo;s official figure is <strong>" + conformance.chart.current.score + "%</strong> — a measurement that got honest, not a checker that got worse.",
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
# In a Basilisk checkout, patch a python/typing checkout, install basilisk from
# the basilisk-python wheel, run the real upstream harness, and write a proof log.
python3 scripts/prepare_typing_conformance_pr.py \
  --typing-repo ../typing \
  --verbose \
  --write-proof
```

The submission workflow lives in [`scripts/prepare_typing_conformance_pr.py`](https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py), with the expected upstream PR files documented in [`docs/typing-conformance-pr.md`](https://github.com/Nimblesite/Basilisk/blob/main/docs/typing-conformance-pr.md). The full annotation rules are in the [python/typing conformance README](https://github.com/python/typing/blob/main/conformance/README.md).
