---
layout: layouts/blog.njk
title: "Retraction: Our Withdrawn Python Typing Conformance Claim"
description: "We retract in full the Python typing conformance claim this post originally made. It was produced by checker logic fitted to the test fixtures, it was not evidence of specification conformance, and it must not be cited."
date: 2026-07-11
dateModified: 2026-08-08
author: Christian Findlay
image: /assets/images/og-image.png
imageAlt: "Basilisk Python type checker and language server; conformance claim retracted"
imageWidth: 1200
imageHeight: 630
tags:
  - Python typing
category: announcements
excerpt: "This post originally announced a conformance result for Basilisk. That result is retracted in full, and the original text has been removed rather than preserved, because leaving it up kept republishing a claim we know to be unsupported."
keywords: basilisk retraction, python typing conformance correction, basilisk conformance withdrawn
faq:
  - q: "Is Basilisk listed in the official python/typing conformance results?"
    a: "No. Basilisk was removed from the official results table at our own request, and we have not asked to be re-listed. We will not submit again until the audit is finished, semantics-preserving mutation testing passes clean, and someone outside the project has reviewed the work."
  - q: "What is Basilisk's current conformance percentage?"
    a: "We do not publish one, and we ask that no figure be attributed to us. Any number we produced before the audit was measured against fixtures our own code had been fitted to, so it did not measure what we said it measured. We expect an honest figure to be lower than the withdrawn one, and we will say so plainly when we can measure it properly."
  - q: "Was the original result an attempt to game the conformance suite?"
    a: "No. It was a verification failure. Our development process named the suite score as the target to build against, and matching source text raises that score faster than real analysis does, so the code drifted that way. Nobody set out to defeat the suite and nothing was concealed from python/typing; the submission ran the suite's own unmodified harness. We did not run the check that would have caught the problem before we published, and that is our responsibility. Basilisk's author has published a personal account at https://www.christianfindlay.com/blog/basilisk-conformance-apology."
  - q: "Should I use Basilisk's type checker in CI?"
    a: "Not yet. Until the audit is finished, do not gate CI on it, do not block a merge with it, and do not read a clean run as a clean codebase. The rest of Basilisk — the language server, refactoring, formatting, debugging, and profiling — does not depend on the rules under audit."
---

> ## This post is retracted in full
>
> It originally announced a Python typing conformance result for Basilisk and used
> that result to compare Basilisk with other type checkers. **Every one of those
> claims is withdrawn** — the score, the ranking, the pass counts, the category
> breakdowns, and every conclusion drawn from them. None of it should be cited,
> quoted, or relied on by anyone, including us.
>
> We have **removed the original text** rather than leave it sitting under a notice.
> A banner on top of an intact announcement still republishes the claim to everyone
> who lands on the page, and search engines and language models keep lifting the
> number back out of it. What follows is the correction.

## What the original post claimed

It announced that Basilisk had been added to the official `python/typing` conformance
results, reported the figure from that run, and presented a leaderboard placing
Basilisk above established checkers from Microsoft, Meta, Astral, and others. It
argued that the result proved Basilisk implemented the typing specification correctly.

All of that is withdrawn. The comparison with other tools is withdrawn along with it:
it was built on our own unreliable row, and it was not a fair statement about anyone
else's work.

## Why the claim was wrong

Parts of Basilisk's checker decided their answers from the **spelling** of source code
rather than its meaning, and that logic had been shaped around the exact contents of
the conformance test files.

The clearest example: type-alias validation ran prefix and substring tests against raw
source text, including a special case for the literal `eval(` — a spelling that has no
standing anywhere in the typing specification and appeared only because one test file
used it. `int("3")` is the identical specification violation and was accepted silently.
Rename an import or reformat a file and the answer changed, even though the typing
behaviour under test had not.

A suite result produced by code developed against that suite's exact fixtures does not
measure specification conformance. The file passes and the rule is not implemented.
That is why the correction is deletion rather than a better score.

## This was a mistake, not an attempt to game the suite

We didn't set out to defeat the conformance suite, and nothing was concealed from
`python/typing`. The submission ran the suite's own unmodified harness, with
Basilisk's default configuration and every specification rule enabled. When the defect was demonstrated, we asked for our own removal.

Our development process named the conformance score as the thing to build against, and matching source text raises that score faster than real analysis does — so that is the direction the code drifted, one plausible-looking rule at a time. Then we published and submitted the result on the
strength of a green run, without ever running the one check that would have exposed
it: does this rule still hold when the same program is spelled differently?

That check did not exist. It still doesn't — building it is part of the remediation.
The conformance suite cannot catch this class of defect by construction, because it is
the very artefact the code was fitted to, so every green run reinforced a conclusion
we had no basis for. We believed the number meant what we said it meant. We were
wrong, and we were wrong because we did not verify it, not because we were trying to
get away with something.

It was also **not us who found it**. It was reported from outside, in
[issue #379](https://github.com/Nimblesite/Basilisk/issues/379), from a public
reproduction. That is its own finding, and we record it as one.

Basilisk's author has written a
[personal account and apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology)
taking responsibility in his own words — that he did not set out to publish a number he
knew was false, did not understand how the rules were passing until it was demonstrated,
and did not verify the code, which he calls his own failure.

## What we have done

- Withdrew the conformance claim and the published benchmark figures.
- Asked `python/typing` to remove Basilisk from the official results
  ([python/typing#2330](https://github.com/python/typing/pull/2330), reverting
  [#2316](https://github.com/python/typing/pull/2316)). It has been removed.
- Published the full
  [integrity audit](https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md)
  — the defect, the method used to find the rest, reproduction commands, and the parts
  still broken — with the remaining work tracked as public issues.
- Started auditing every rule and **deleting** the ones that decide from source text,
  leaving a failing test behind each removal so the gap is visible rather than hidden.

## What is true today

**We publish no conformance figure, and we ask that none be attributed to us.** Not the
withdrawn one, not a revised one. A number is not what is wrong here, and putting a new
one out before the audit finishes would repeat the mistake.

**Do not put Basilisk's type checker in your pipeline yet.** It still contains code that
isn't doing real type checking, so it can be wrong in both directions — a false error on
correct code, or silence where there is a genuine bug.

**The rest of Basilisk is unaffected.** The language server, refactoring, formatting,
integrated debugging, and profiling do not rest on the rules under audit. Sharpening
those, and removing anything that could hand you a misleading result, is the work in
front of us.

Basilisk is MIT-licensed and provided **as is, without warranty of any kind**.

## If you cited this post

Please update or remove the citation. If you are holding a screenshot, a ranking table,
or a figure that traces back here, it is withdrawn. We would rather carry the cost of
the correction than have the number keep circulating.

Read the [full correction](/docs/conformance/) for the audit scope, the deletion policy,
and the bar any future result has to clear before we publish anything again.
