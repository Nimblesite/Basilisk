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

All rules are enabled by default. There is no opt-in.

| Group | Codes | Description |
|---|---|---|
| [Missing Annotations](/docs/rules/missing-annotations/) | E0001–E0009 | Unannotated parameters, return types, variables, and attributes |
| [Type Safety](/docs/rules/type-safety/) | E0010–E0025 | Type mismatches, incorrect annotations, unsound type usage |
| [Ownership Safety](/docs/rules/ownership-safety/) | E0030–E0035 | Mojo-inspired ownership violations |
| [Immutability](/docs/rules/immutability/) | E0040–E0043 | Mutation of immutable parameters and `Final` variables |
| [Structural Discipline](/docs/rules/structural-discipline/) | E0050–E0054 | Dynamic attributes, missing `__init__`, sealed class violations |
| [Coercion Safety](/docs/rules/coercion-safety/) | E0060–E0063 | Implicit numeric and type coercions |
| [Optional Safety](/docs/rules/optional-safety/) | E0070–E0073 | Unsafe access on `Optional` values |
| [Unused Code](/docs/rules/unused-code/) | W0080–W0089 | Unused imports, variables, functions, and unreachable branches |
| [Code Quality](/docs/rules/code-quality/) | W0090–W0099 | Suppression comments, deprecated APIs, mutable defaults |
