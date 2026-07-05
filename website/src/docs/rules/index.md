---
layout: layouts/docs.njk
title: "Basilisk Diagnostic Rules — All BSK Error and Warning Codes"
description: "Reference for every Basilisk diagnostic code and rule — BSK-E errors and BSK-W warnings from the strict Python type checker, with causes and fixes for each."
keywords: basilisk rules, type errors, BSK-E, BSK-W, diagnostic codes
date: 2026-02-28
dateModified: 2026-05-30
author: The Basilisk Project
eleventyNavigation:
  key: Rules
  order: 5
---

# Diagnostic Rules

Every Basilisk diagnostic has a unique code in the format `BSK-EXXXX` (error) or `BSK-WXXXX` (warning).

The PEP typing-spec rules run by default; Basilisk's own opt-in house-style rules stay off until you switch them on. Dial any rule up or down per-file or per-path from your editor or `pyproject.toml` — strict is the default, not a cage.

Basilisk ships **{{ ruleStats.pep }} PEP typing-spec rules** — the set the [official Python typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) grades (currently **{{ conformance.scorePct }}%**, {{ conformance.pass }} / {{ conformance.total }} (errors+warnings, strictest); target 100% — [how we measure](/docs/conformance/)) — plus **{{ ruleStats.optIn }} opt-in house-style rules** that are off by default and never counted toward that score. Together, **{{ ruleStats.total }} diagnostic codes** ({{ ruleStats.errors }} errors, {{ ruleStats.warnings }} warnings) span the full Python typing surface — generics, protocols, dataclasses, TypedDicts, overloads, literals, enums, and more. The two foundational groups have worked examples:

| Group | Codes | Description |
|---|---|---|
| [Missing Annotations](/docs/rules/missing-annotations/) | E0001–E0009 | Unannotated parameters, return types, variables, and attributes |
| [Type Safety](/docs/rules/type-safety/) | E0010–E0029 | Type mismatches, incorrect annotations, unsound type usage |

> **Roadmap:** Mojo-inspired ownership and immutability analysis is planned for a future release. It is not yet part of the shipping rule set.

## Complete diagnostic reference

Every code the checker emits — generated from the checker source
(`scripts/gen_rules_reference.py`), so it never drifts. **Each code links to its
own page** with a full explanation and fix, the same page the CLI sends you to
(`see: https://www.basilisk-python.dev/errors/BSK-XXXX`). Browse them all in the
[error reference](/errors/).

<table class="rules-table">
<thead><tr><th>Code</th><th>Description</th></tr></thead>
<tbody>
{%- for rule in rules %}
<tr><td><a href="/errors/{{ rule.code }}/"><code>{{ rule.code }}</code></a></td><td>{{ rule.summaryHtml | safe }}</td></tr>
{%- endfor %}
</tbody>
</table>
