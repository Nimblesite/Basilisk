---
layout: layouts/docs.njk
title: Code Quality — W0090–W0099
description: "Basilisk code quality warnings — unnecessary type:ignore comments, deprecated APIs, Python 2 style type comments, assertions with side effects, and mutable default arguments. BSK-W0090 through W0099."
keywords: basilisk, code quality, deprecated, mutable default, BSK-W0090, BSK-W0099
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Code Quality
  parent: Rules
  order: 9
---

# Code Quality — W0090–W0099

Warnings for patterns that are legal but problematic.

← [Unused Code](/docs/rules/unused-code/) →

| Code | Description |
|---|---|
| BSK-W0090 | Unnecessary `type: ignore` comment — no error at this location |
| BSK-W0091 | Use of deprecated API |
| BSK-W0092 | `type:` comment instead of annotation syntax (Python 2 style) |
| BSK-W0093 | `assert` statement with side effects — assertions can be disabled |
| BSK-W0094 | Mutable default argument |
| BSK-W0095 | Suppression comment without reason |
