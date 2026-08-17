---
layout: layouts/docs.njk
title: "Basilisk Diagnostic Rules — Python Typing & Opt-In Rule Tags"
description: "Browse every Basilisk diagnostic by its authoritative tags: Python typing-spec rules enabled by default and Basilisk-specific rules enabled explicitly by tag."
keywords: basilisk rules, python typing rules, strictness rules, type errors, BSK-E, BSK-W, diagnostic codes
date: 2026-02-28
dateModified: 2026-07-11
author: The Basilisk Project
eleventyNavigation:
  key: Rules
  order: 5
---
{% from "components/rules.njk" import groupGrid with context %}

# Diagnostic rules

Basilisk organises rules by the same flat tags the checker uses—not by arbitrary code ranges. Every rule has exactly one provenance tag:

- `pep` identifies the **{{ ruleTagGroups.counts.pep }} core rules enabled by default**, including the rules measured by the [official Python typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html).
- `basilisk` identifies the **{{ ruleTagGroups.counts.basilisk }} Basilisk-specific rules that are off by default** and activate only when a project opts into one of their descriptive tags.

Each diagnostic links to a permanent `/errors/.../` explanation—the same URL the CLI prints when a rule fires.

## Basilisk rules (opt-in)

Use these tags when you want checking beyond the typing specification. A rule can appear under more than one descriptive tag; for example, a dependency rule may also be tagged `imports`.

{{ groupGrid(ruleTagGroups.basilisk, "en") }}

## Python typing-spec rules

These rules form the default checker surface. Their category tags come directly from the `python/typing` conformance vocabulary; cross-cutting checks that span categories are collected under **Cross-cutting core**.

{{ groupGrid(ruleTagGroups.pep, "en") }}

## Complete diagnostic reference

Browse the [tag-grouped error reference](/errors/) or jump directly to any canonical page at `/errors/CODE/`. Rule selection and severity overrides are documented in the [`pyproject.toml` configuration reference](/docs/configuration/).
