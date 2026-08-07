---
layout: layouts/blog.njk
title: "Retracted: Basilisk's Former Typing Conformance Result"
description: "Retraction of Basilisk's former Python typing conformance claim, why the result was untrustworthy, and why the affected rules are being deleted rather than repaired."
date: 2026-07-11
dateModified: 2026-08-06
author: Christian Findlay
image: /assets/images/og-image.png
imageAlt: "Basilisk Python type checker and language server; results under integrity review"
imageWidth: 1200
imageHeight: 630
tags:
  - Python typing
category: announcements
excerpt: "Basilisk has retracted its former conformance claim and requested removal from the official results. This post is retained only as a record of the withdrawn announcement."
keywords: python type checker, python typing conformance, python/typing conformance results, basilisk, mypy, pyright, ty, pyrefly, zuban, pep conformance, strict typing
faq:
  - q: "Which Python type checker has the highest conformance score?"
    a: "Basilisk is not currently listed in the official python/typing results. Its former result is withdrawn and its actual percentage is temporarily unknown while every rule is audited and the ones that matched source text are deleted. Check the live official table for currently listed tools."
  - q: "What is the python/typing conformance suite?"
    a: "It is the official test suite maintained by the Python Typing community. Its harness records how a checker behaves on the suite's exact fixtures. That is valuable evidence, but a raw suite result alone does not establish faithful implementation of the full specification; mutation robustness and independent off-suite cases are also required."
  - q: "Is a 100% conformance score the same as being the best type checker?"
    a: "No. A suite score describes the covered fixtures; it is not proof of specification correctness by itself, as Basilisk's retraction demonstrates. It also does not capture editor integration, error quality, ecosystem support, or independently validated performance."
  - q: "How is Basilisk's conformance score measured?"
    a: "There is no current Basilisk conformance score. A future result will require the unmodified python/typing harness, semantics-preserving mutation testing, and independent off-suite cases derived from the specification, and only after the audit has finished removing rules that decided from source text rather than resolved symbols."
---

> **Retraction — 6 August 2026:** We withdraw every conformance claim in this post. Basilisk's source contained logic fitted to the exact conformance fixtures, so the former perfect result did not establish specification conformance. We asked for Basilisk to be removed from the official results table, and it has been removed. The current percentage is temporarily unknown, and we are not trying to restore it: we are auditing every rule and deleting the ones that matched the spelling of code rather than its meaning, which will push the number lower before anything improves. The original article is retained below only as a public record; its score, ranking, pass counts, and conclusions must not be relied on. Read the [full correction](/docs/conformance/).

Python has a genuinely good type system now, and most developers still do not realize it. A Python type checker works a lot like the TypeScript compiler. Type-checked Python is to regular Python what TypeScript is to JavaScript. The annotations have been in the language for a decade, the specification is mature, and the tooling has caught up.

The open question was never whether Python's type system was good enough. It was how faithfully any given tool actually implements it.

At publication, we believed we had an objective answer for Basilisk. It had been added to this [pinned snapshot of the official python/typing conformance results]({{ conformanceOfficial.historical.snapshot.snapshotUrl }}), where that run reported {{ conformanceOfficial.historical.basilisk.pct }}% ({{ conformanceOfficial.historical.basilisk.passLabel }} of {{ conformanceOfficial.historical.basilisk.total }} tests). That result is now withdrawn.

We were proud of that result. The integrity audit showed that conclusion was wrong.

## Why the conformance suite is the referee that matters

A tool does not get to grade its own homework. Every type checker author will tell you their tool is excellent. That is not evidence.

The [python/typing conformance suite](https://github.com/python/typing/tree/main/conformance) is the closest thing the Python ecosystem has to an objective referee. It is maintained by the Python Typing community, it encodes the actual typing specification as a set of test files, and it runs every participating checker through the same tests with the same harness. Nobody grades themselves. The suite grades all of them, together, on one run.

We treated that as making the result meaningful. The suite did produce the number with its shared harness, but our code had been fitted to exact fixture text. The measurement therefore did not support the conclusion we drew from it.

Basilisk was added to that run in [python/typing pull request #2316](https://github.com/python/typing/pull/2316), "Add Basilisk to conformance results," merged on July 6, 2026. We later requested removal after retracting the result, and Basilisk no longer appears in the live table.

## The historical board snapshot we published

The following is the leaderboard snapshot that accompanied the original announcement on {{ conformanceOfficial.historical.snapshot.dateLabel }}. It is not current, and Basilisk's row is withdrawn. Use the [live official results]({{ conformanceOfficial.historical.snapshot.source }}) for tools that remain listed.

| Rank | Type checker | Backed by | Conformance |
|---|---|---|---|
{%- for t in conformanceOfficial.historical.ranked %}
| {{ t.rank }} | [{{ t.name }} {{ t.version }}]({{ t.resultsUrl }}) | {{ t.org | default("Independent") }} | **{{ t.pct }}%** ({{ t.passLabel }}/{{ t.total }}) |
{%- endfor %}

A few things are worth saying plainly about that table, because the company Basilisk is keeping is serious.

[Pyright](https://github.com/microsoft/pyright) is developed by Microsoft. [Pyrefly](https://github.com/facebook/pyrefly) is built by Meta. [ty](https://github.com/astral-sh/ty) is built by Astral, the team behind Ruff and uv, which [has agreed to join OpenAI](https://openai.com/index/openai-to-acquire-astral/) (a deal announced in March 2026 and, at announcement, still subject to regulatory approval and customary closing conditions). [mypy](https://github.com/python/mypy) is the original, created by Jukka Lehtosalo and developed heavily at Dropbox. [zuban](https://github.com/zubanls/zuban) is written by David Halter, the author of Jedi. [pycroscope](https://github.com/JelleZijlstra/pycroscope) is maintained by CPython core developer Jelle Zijlstra.

At publication, we used this snapshot to place Basilisk above the other tools. That comparison is withdrawn because Basilisk's result was not robust.

The original post presented the snapshot as proof that a small independent tool sat at the top of a board containing much larger teams. That presentation is part of the withdrawn claim.

## What 100% does and does not mean

Here is the part where we argue against our own headline, because you deserve the honest version.

The python/typing maintainers put a caveat right at the top of the results page, and we agree with it completely:

> "While specification conformance is important for the ecosystem, we don't recommend using it as the primary basis for choosing a type checker. It is not representative of many of the things users typically care about." ([python/typing conformance results]({{ conformanceOfficial.historical.snapshot.source }}))

Read that twice. The people who built the suite are telling you not to treat their own scoreboard as the only thing that matters. That is the right position, and we are not going to pretend otherwise to make Basilisk look better.

The original post tried to explain what we believed a perfect conformance score meant. The integrity audit invalidated the central claim.

**What we claimed it was:** proof that Basilisk judged code correctly against the typing specification. That inference was wrong. Passing the exact suite did not establish a general implementation when parts of the checker matched the fixtures' text.

**What it is not:** a claim that Basilisk is automatically the best choice for your project. Conformance does not measure how fast a checker runs, how good its error messages are, how well it integrates with your editor, or how mature its ecosystem is. Those things matter enormously, and on some of them the older tools have years of head start.

Conformance is one input. It happens to be the input that decides whether you can trust the rest. But it is not the whole decision, and anyone who tells you a single number settles the question is selling something.

## Why we chased the number anyway

If conformance is not the whole story, why did we make 100% a hard requirement rather than a nice-to-have?

Because the alternative is a checker that is confidently wrong some of the time, and a checker that is confidently wrong is worse than no checker at all. The problem with Python typing was never the syntax. The problem was enforcement. A type hint that is never checked is a comment. A type hint that is checked by a tool with gaps is a comment that occasionally lies to you.

Basilisk enables its typing-spec rules by default, with no `--strict` flag to remember. We claimed the old score proved those rules implemented the specification correctly. It did not, and the rules that cannot show they analyse code are being deleted rather than repaired.

## How the withdrawn score was produced

We measure this the boring, reproducible way, because that is the only kind of measurement worth publishing.

The withdrawn number came from the suite's unmodified harness, run against the default-configuration Basilisk CLI with every specification rule enabled. That procedure reproduced the result, but it could not reveal that parts of the implementation were fitted to the exact tests. Future publication will therefore require both the official harness and mutation-based robustness checks.

That is a deliberate design choice, and it maps to a rule we hold for everything we ship: self-measured metrics are only worth anything if they are reproducible and measured by a neutral party. The conformance suite is that neutral party. We just make sure our tool shows up and runs.

## Try it, and try to break it

You can read the current correction and remediation plan on our [conformance page](/docs/conformance/), and see that Basilisk is no longer listed on the [python/typing results page]({{ conformanceOfficial.historical.snapshot.source }}).

Point Basilisk at your own code and report disagreements on [GitHub](https://github.com/Nimblesite/Basilisk/issues). The old score cannot stand in for that real-world scrutiny. A replacement result will be published only after the clean implementation survives broader regression cases and semantics-preserving mutations.

Python's type system has been good enough to trust for a while. Now the tooling can be too.
