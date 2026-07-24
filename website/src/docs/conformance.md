---
layout: layouts/docs.njk
title: "Basilisk Scores 100% on the Official Python Typing Conformance Suite"
description: "Basilisk is the only Python type checker with a perfect 100% score — published on the official python/typing conformance results page, ahead of Pyright, mypy, Pyrefly and ty. Here's the proof and how it's measured."
keywords: pep conformance, python typing conformance results, 100% conformant type checker, best python type checker, basilisk conformance score, python/typing results
date: 2026-06-23
dateModified: 2026-07-07
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 10
---
{% from "conformance-chart.njk" import chart %}

# Basilisk scores a perfect 100%

Basilisk is the **only Python type checker with a perfect {{ conformanceOfficial.byId.basilisk.pct }}% score** on the [**official `python/typing` conformance results**](https://github.com/python/typing/blob/main/conformance/results/results.html) — and it is **published right there on the Python typing repository's own results page**, graded on the same single run as every other checker.

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>Official python/typing results ↗</strong></a>
  <a href="{{ conformanceOfficial.snapshot.prUrl }}" target="_blank" rel="noopener">The PR that added us ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance README ↗</a>
</p>

## The official leaderboard

Every score below comes from **one identical run** of the [official `python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) — the same suite and scorer the typing community uses to grade every checker. Basilisk tops it, and is the **only tool on the board at a perfect score**.

<div class="table-wrapper">
<table>
<thead><tr><th>Tool</th><th>Backer</th><th>Official conformance</th></tr></thead>
<tbody>
{%- for t in conformanceOfficial.ranked %}
<tr{% if t.id == "basilisk" %} class="conf-row-basilisk"{% endif %}>
  <td>{% if t.id == "basilisk" %}<strong>Basilisk</strong>{% else %}{{ t.name }}{% endif %}</td>
  <td>{{ t.org or "independent" }}</td>
  <td><a href="{{ t.resultsUrl }}" target="_blank" rel="noopener">{% if t.id == "basilisk" %}<strong>{{ t.pct }}% ({{ t.passLabel }}/{{ t.total }})</strong>{% else %}{{ t.pct }}%{% endif %}</a></td>
</tr>
{%- endfor %}
</tbody>
</table>
</div>

<p class="conf-note">Snapshot of <a href="{{ conformanceOfficial.snapshot.source }}" target="_blank" rel="noopener">results.html</a> at <a href="{{ conformanceOfficial.snapshot.prUrl }}" target="_blank" rel="noopener">python/typing@<code>{{ conformanceOfficial.snapshot.sha }}</code></a> ({{ conformanceOfficial.snapshot.dateLabel }}). These figures drift as the other tools improve, so every cell links to that tool's <strong>live</strong> results folder — check the current number yourself.</p>

## How it's measured

We don't score ourselves against our own yardstick. The number above is produced by the **official `python/typing` harness**, run unmodified against the `basilisk` CLI built straight from the current checkout (`cargo build --release`), in its **default configuration**, with **every PEP conformance rule on and nothing else configured** — the same binary every install channel ships. A file passes only when the harness's diff is empty: every required error reported, and **nothing** reported on a line the suite doesn't mark. We count every diagnostic the checker emits — errors *and* warnings — so a single false positive fails the whole file.

Today that is **{{ conformance.scorePct }}%** — **{{ conformance.pass }} of {{ conformance.total }}** test files passing, {{ conformance.caught }} required errors caught, **{{ conformance.fp }} false positives**, **{{ conformance.missed }} missed errors**. We run in lock step with `python/typing@main` (graded at [`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}, {{ conformance.commitDate }}{% endif %}); a ratchet gate keeps the score from ever regressing, and an upstream test we fail blocks merge and release.

Basilisk's **opt-in house-style rules** (require-annotation, redundant-annotation, missing-`@override`, explicit-`Any`) never run during scoring — a fresh install runs none of them, and enabling them would only *lower* the score, since the spec treats an unannotated value as *inferred*, not an error. "Stricter than the spec" and "conformant to the spec" are different goals; this score measures only the second.

### Reproduce it yourself

Basilisk is a **registered checker in the official suite** — `BasiliskTypeChecker`
lives in `python/typing`'s [`conformance/src/type_checker.py`](https://github.com/python/typing/blob/main/conformance/src/type_checker.py) —
so you run the real harness directly, with nothing to patch:

```bash
# Clone python/typing FRESH, run its OWN harness against the basilisk binary,
# and regenerate conformance/conformance_status.csv from the real results.
python3 conformance/run_conformance.py --bin target/release/basilisk
```

Or drive the upstream harness by hand against any `basilisk` on your PATH:

```bash
git clone --depth 1 https://github.com/python/typing
BASILISK_BIN=$(which basilisk) python typing/conformance/src/main.py --only-run basilisk
```

The runner lives in [`conformance/run_conformance.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/run_conformance.py); it clones the suite fresh, runs the unmodified upstream harness, and never scores anything itself.

## How the score got honest

We'd rather say this plainly than paper over it. An earlier in-repo script inflated the figure by **excluding some diagnostic codes from the diff and not counting false positives at all**. We threw it out and adopted the official `python/typing` scoring semantics on the real default CLI. The chart is read straight from the **git history of `conformance/conformance_status.csv`** at build time — one point per commit that changed it, including that correction.

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
</content>
