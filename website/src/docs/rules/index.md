---
layout: layouts/docs.njk
title: "Basilisk Diagnostic Rules — All BSK Error and Warning Codes"
description: "Complete reference for all Basilisk diagnostic codes (BSK-E errors and BSK-W warnings). Missing annotations, type safety, ownership, immutability, and more."
keywords: basilisk rules, type errors, BSK-E, BSK-W, diagnostic codes
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Rules
  order: 5
---

# Diagnostic Rules

Every Basilisk diagnostic has a unique code in the format `BSK-EXXXX` (error) or `BSK-WXXXX` (warning).

Rules are enabled by default. You can dial individual rules down per-file or per-path from your editor or `pyproject.toml` — strict is the default, not a cage.

Basilisk implements 150+ diagnostic codes spanning the full Python typing surface (generics, protocols, dataclasses, TypedDicts, overloads, literals, and more), driven by the [official Python typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html). The two foundational groups are documented below; the complete set is enforced by the checker.

| Group | Codes | Description |
|---|---|---|
| [Missing Annotations](/docs/rules/missing-annotations/) | E0001–E0009 | Unannotated parameters, return types, variables, and attributes |
| [Type Safety](/docs/rules/type-safety/) | E0010–E0029 | Type mismatches, incorrect annotations, unsound type usage |

> **Roadmap:** Mojo-inspired ownership and immutability analysis is planned for a future release. It is not yet part of the shipping rule set.
