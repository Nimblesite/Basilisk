---
layout: layouts/docs.njk
title: "Basilisk Is Auditing and Deleting Its Checker Rules"
description: "Basilisk withdrew its Python typing conformance claim and is now auditing every rule, deleting the ones that match source text instead of doing real type checking."
keywords: basilisk conformance correction, python typing conformance, python/typing results, mutation testing
date: 2026-06-23
dateModified: 2026-08-08
author: The Basilisk Project
eleventyNavigation:
  key: Conformance
  order: 10
---

# We are auditing the checker and deleting what doesn't hold up

<p class="bench-caveat"><strong>Correction:</strong> Basilisk has retracted its former perfect-score claim. The result was not a trustworthy measure of specification conformance. We asked for Basilisk to be removed from the <code>python/typing</code> results table, and it <a href="https://github.com/python/typing/blob/main/conformance/results/results.html">has been removed</a>. Basilisk's current conformance percentage is <strong>temporarily unknown</strong>, and we are not trying to restore it.</p>

We found checker logic fitted to the exact contents of conformance test files rather than implementing the typing specification generally. Those rules matched the *spelling* of code rather than its meaning: type-alias validation used prefixes and substrings taken from raw source text, including a special case for `eval(` purely because that spelling appeared in one test file. Rename an import or reformat a file and the answer changed, even though the typing behavior being tested had not.

A passing result from code developed against the exact fixtures is not evidence, so the fix is not a better score.

## This was a mistake, not an attempt to game the suite

We didn't set out to defeat the conformance suite. What actually happened is duller. Our development process named the conformance score as the thing to build against, and matching source text raises that score faster than real analysis does, so that is the direction the code drifted — one plausible-looking rule at a time. We then published and submitted on the strength of a green run, without ever running the one check that would have exposed it: does this rule still hold when the same program is spelled differently? That check did not exist, and building it is part of the remediation below. The suite cannot catch this class of defect by construction, because it is the artefact the code was fitted to, so every green run reinforced a conclusion we had no basis for.

We believed the number meant what we said it meant. We were wrong, and we were wrong because we failed to verify it. Basilisk's author has published a [personal account and apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology) taking responsibility in his own words.

We also did not find this ourselves. It was reported from outside, in [issue #379](https://github.com/Nimblesite/Basilisk/issues/379), with a public reproduction. That is its own finding, and we record it as one.

## What we are doing

**We are auditing every rule and deleting the ones that don't do real type checking.** Not rewriting them, not patching them, not marking them TODO — deleting them, and leaving a failing test behind so the gap is visible rather than hidden. A rule stays only if it decides from the resolved syntax tree and returns the same diagnostics when the same program is spelled differently.

The consequences are deliberate, and we would rather state them up front than have you discover them:

- **Basilisk gets smaller before it gets better.** Expect fewer rules and fewer diagnostics.
- **The conformance number will fall.** That is the correct outcome of removing logic that was never doing the analysis, and we will report each drop rather than avoid it.
- **A failing test is worth more to us than a passing fixture** that was carried by code which doesn't analyse anything. The first is an accurate record of what Basilisk cannot do; the second is a claim that it can.

What is left will be code that is honest about what it does — nothing else.

**Where a rule can't be made reliable in a straightforward way, we will depend on a different, established type checker rather than ship our own unreliable version of it.** An answer from an engine that has earned trust is worth more to you than a Basilisk-branded one that hasn't. No replacement percentage gets published until it survives the robustness testing described below.

Type checking is one part of Basilisk. The rest — language server, refactoring, formatting, integrated debugging, profiling, and the editor extensions — does not rest on the rules under audit, and that is what we are sharpening while the audit runs: make the parts that are genuinely useful solid, and remove anything that could hand you a misleading result.

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>Current python/typing results ↗</strong></a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md" target="_blank" rel="noopener">Full integrity audit ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/379" target="_blank" rel="noopener">Original bug report ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/408" target="_blank" rel="noopener">Integrity remediation tracker ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python typing spec ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">Conformance README ↗</a>
</p>

## Scope of the audit

The review covers every place a narrow fixture could have stood in for a general implementation: source-text predicates and substring matching, hard-coded symbol spellings, rules organised around a test file rather than a specification concept, duplicated logic, and accept-everything fallbacks standing in for checks that were never written.

Each finding is handled the same way — a test that fails because of the code, then the code is removed, then the removal is recorded. Nothing is quietly repaired in place, because a repair preserves the claim that the rule worked.

This is active remediation, not an indefinite withdrawal. If a defensible result is lower than the former claim, we publish the lower result.

## The new publication bar

A future conformance result must satisfy all of these checks:

1. Run the official, unmodified `python/typing` harness against Basilisk's default configuration.
2. Apply AST-preserving mutations such as consistent renaming of type variables and equivalent spelling changes. A rule is not accepted if those changes move its result.
3. Pass independent off-suite cases derived from the typing specification and real-world code rather than from the upstream fixture text.
4. Add regression and mutation tests for every test-specific implementation found by the audit.
5. Publish the robustness and off-suite results alongside the suite percentage and make the methodology reproducible.
6. Pass an audit by someone outside this project before Basilisk is submitted to `python/typing` again.

Until that work is complete, old conformance tables, charts, category scores, pass counts, and false-positive totals are withdrawn and should not be cited as Basilisk's current state. We are not quoting a current figure either — a number is not what is wrong here, and publishing a new one before the audit finishes would repeat the mistake.

## Related performance figures

The same review failure means our published benchmark figures also require revalidation. They are retained only as a clearly labelled historical record on the [benchmarks page](/docs/benchmarks/) and must not be used to compare Basilisk with other tools. New performance figures will be published only after the methodology and results have passed the integrity review.
