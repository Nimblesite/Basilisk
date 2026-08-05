---
layout: layouts/docs.njk
title: "Basilisk Conformance Results Are Withdrawn"
description: "Basilisk has withdrawn its former Python typing conformance claim. Its current percentage is temporarily unknown while affected logic is rebuilt and stress-tested beyond the suite."
keywords: basilisk conformance correction, python typing conformance, python/typing results, mutation testing
date: 2026-06-23
dateModified: 2026-08-06
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 10
---

# Conformance results withdrawn

<p class="bench-caveat"><strong>Correction:</strong> Basilisk has retracted its former perfect-score claim. The result was not a trustworthy measure of specification conformance. We asked for Basilisk to be removed from the <code>python/typing</code> results table, and it <a href="https://github.com/python/typing/blob/main/conformance/results/results.html">has been removed</a>. Basilisk's current conformance percentage is <strong>temporarily unknown</strong>.</p>

We found checker logic fitted to the exact contents of conformance test files rather than implementing the typing specification generally. For example, type-alias validation used prefixes and substrings from raw source text, including a special case for `eval(` because that spelling appeared in one test. Equivalent, valid mutations of the suite could therefore change Basilisk's result even though the typing behavior being tested had not changed.

The official suite remains valuable, but a passing result from code developed against the exact fixtures is not enough evidence. We will not publish a replacement percentage until the affected logic has been reimplemented cleanly and shown to survive robustness testing.

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>Current python/typing results ↗</strong></a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md" target="_blank" rel="noopener">Full integrity audit ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/379" target="_blank" rel="noopener">Original bug report ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/408" target="_blank" rel="noopener">Integrity remediation tracker ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance README ↗</a>
</p>

## What is happening now

The offending implementation is being removed, and the affected behavior is being rebuilt from the specification and structured syntax rather than from test-file text. The review also covers similar source-text predicates, duplicated logic, permissive fallbacks, and other places where a narrow fixture could have stood in for a general implementation.

This is active remediation, not an indefinite withdrawal. We expect to establish a defensible result after the clean implementation and validation work is complete. If that result is lower than the former claim, we will publish the lower result.

## The new publication bar

A future conformance result must satisfy all of these checks:

1. Run the official, unmodified `python/typing` harness against Basilisk's default configuration.
2. Apply AST-preserving mutations such as consistent renaming of type variables and equivalent spelling changes. A rule is not accepted if those changes move its result.
3. Add regression and mutation tests for every test-specific implementation found by the audit.
4. Publish the robustness result alongside the suite percentage and make the methodology reproducible.

Until that work is complete, old conformance tables, charts, category scores, pass counts, and false-positive totals are withdrawn and should not be cited as Basilisk's current state.

## Related performance figures

The same review failure means our published benchmark figures also require revalidation. They are retained only as a clearly labelled historical record on the [benchmarks page](/docs/benchmarks/) and must not be used to compare Basilisk with other tools. New performance figures will be published only after the methodology and results have passed the integrity review.
