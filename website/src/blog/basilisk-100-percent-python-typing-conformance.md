---
layout: layouts/blog.njk
title: "Basilisk Hits 100% on the Python Typing Conformance Suite"
description: "Basilisk is now on the official python/typing conformance results at a perfect 100%, the only Python type checker to reach it. Here is what that means."
date: 2026-07-11
author: Christian Findlay
image: /assets/images/blog/basilisk-100-conformance.png
imageAlt: "Python type checker conformance leaderboard showing Basilisk at a perfect 100 percent score"
imageWidth: 1200
imageHeight: 675
tags:
  - posts
  - Python typing
  - conformance
  - Basilisk
category: announcements
excerpt: "Basilisk joined the official python/typing conformance results this week, and it landed at a perfect score. It is the only Python type checker on the board at 100%. Here is what that number actually means, who else is on the board, and why we are not going to oversell it."
keywords: python type checker, python typing conformance, python/typing conformance results, basilisk, mypy, pyright, ty, pyrefly, zuban, pep conformance, strict typing
faq:
  - q: "Which Python type checker has the highest conformance score?"
    a: "On the official python/typing conformance results, Basilisk 0.27.0 scores a perfect 100% (141 of 141 tests). It is the only checker on the board at 100%. zuban, Pyrefly, and Pyright follow closely, all above 96%. Competitor scores move as those tools improve, so check each tool's live results folder for the current figure."
  - q: "What is the python/typing conformance suite?"
    a: "It is the official test suite maintained by the Python Typing community that measures how faithfully a type checker implements the Python typing specification. Each checker is run against the same set of tests and graded by the suite's own harness. The results are published at github.com/python/typing under conformance/results."
  - q: "Is a 100% conformance score the same as being the best type checker?"
    a: "No. The python/typing maintainers explicitly say conformance should not be the primary basis for choosing a type checker, because it does not capture speed, editor integration, error message quality, or ecosystem support. Conformance measures spec correctness only. It is one important input, not the whole decision."
  - q: "How is Basilisk's conformance score measured?"
    a: "Basilisk is scored by the python/typing suite's own unmodified harness, running against the default-config Basilisk CLI with every spec rule on. There is no vendored scorer and no special configuration. The score is what a user gets out of the box, graded by the same code that grades every other checker on the board."
---

Python has a genuinely good type system now, and most developers still do not realize it. A Python type checker works a lot like the TypeScript compiler. Type-checked Python is to regular Python what TypeScript is to JavaScript. The annotations have been in the language for a decade, the specification is mature, and the tooling has caught up.

The open question was never whether Python's type system was good enough. It was how faithfully any given tool actually implements it.

This week we got an objective answer for Basilisk. It was added to the [official python/typing conformance results]({{ conformanceOfficial.snapshot.source }}), and it landed at a perfect {{ conformanceOfficial.basilisk.pct }}% ({{ conformanceOfficial.basilisk.passLabel }} of {{ conformanceOfficial.basilisk.total }} tests). It is the only type checker on the board at {{ conformanceOfficial.basilisk.pct }}%.

We are proud of that. We are also not going to oversell it, and the rest of this post explains both halves of that sentence.

## Why the conformance suite is the referee that matters

A tool does not get to grade its own homework. Every type checker author will tell you their tool is excellent. That is not evidence.

The [python/typing conformance suite](https://github.com/python/typing/tree/main/conformance) is the closest thing the Python ecosystem has to an objective referee. It is maintained by the Python Typing community, it encodes the actual typing specification as a set of test files, and it runs every participating checker through the same tests with the same harness. Nobody grades themselves. The suite grades all of them, together, on one run.

That is what makes the result meaningful. When Basilisk shows {{ conformanceOfficial.basilisk.pct }}% on that page, it is not our claim. It is the suite's measurement, produced by [the same harness](https://github.com/python/typing/blob/main/conformance/README.md) that measures everyone else.

Basilisk was added to that run in [python/typing pull request #2316](https://github.com/python/typing/pull/2316), "Add Basilisk to conformance results," merged on July 6, 2026. From that point on, Basilisk is measured in public, on the same terms as every other tool, and you can check the number yourself any time.

## The board, as it stands

Here is the current leaderboard, transcribed from the [official results]({{ conformanceOfficial.snapshot.source }}) as published on {{ conformanceOfficial.snapshot.dateLabel }}. Every score links to that tool's live results folder, because these numbers move as each tool improves, and you should always be able to check the current figure rather than trust a snapshot.

| Rank | Type checker | Backed by | Conformance |
|---|---|---|---|
{%- for t in conformanceOfficial.ranked %}
| {{ t.rank }} | [{{ t.name }} {{ t.version }}]({{ t.resultsUrl }}) | {{ t.org | default("Independent") }} | **{{ t.pct }}%** ({{ t.passLabel }}/{{ t.total }}) |
{%- endfor %}

A few things are worth saying plainly about that table, because the company Basilisk is keeping is serious.

[Pyright](https://github.com/microsoft/pyright) is developed by Microsoft. [Pyrefly](https://github.com/facebook/pyrefly) is built by Meta. [ty](https://github.com/astral-sh/ty) is built by Astral, the team behind Ruff and uv, which [has agreed to join OpenAI](https://openai.com/index/openai-to-acquire-astral/) (a deal announced in March 2026 and, at announcement, still subject to regulatory approval and customary closing conditions). [mypy](https://github.com/python/mypy) is the original, created by Jukka Lehtosalo and developed heavily at Dropbox. [zuban](https://github.com/zubanls/zuban) is written by David Halter, the author of Jedi. [pycroscope](https://github.com/JelleZijlstra/pycroscope) is maintained by CPython core developer Jelle Zijlstra.

These are teams with real headcount, real budgets, and deep expertise. Several of them are excellent, and the scores show it. zuban, Pyrefly, and Pyright are all above 96%, which is genuinely hard to achieve. None of them is at 100%. Basilisk is.

We do not say that to spike the ball. We say it because it is the fact the suite reports, and because it is a strange and good thing that a small independent tool sits at the top of a board that includes three of the largest software companies in the world. That is the whole promise of an open, shared conformance suite: it does not care who is behind a tool. It only cares whether the code is correct.

## What 100% does and does not mean

Here is the part where we argue against our own headline, because you deserve the honest version.

The python/typing maintainers put a caveat right at the top of the results page, and we agree with it completely:

> "While specification conformance is important for the ecosystem, we don't recommend using it as the primary basis for choosing a type checker. It is not representative of many of the things users typically care about." ([python/typing conformance results]({{ conformanceOfficial.snapshot.source }}))

Read that twice. The people who built the suite are telling you not to treat their own scoreboard as the only thing that matters. That is the right position, and we are not going to pretend otherwise to make Basilisk look better.

So let us be precise about what a perfect conformance score is and is not.

**What it is:** proof that when Basilisk judges your code against the typing specification, its judgment is correct. A checker that does not implement a spec feature cannot reason about code that uses it. It either misses a real error or invents a false one. On the conformance run, Basilisk caught every required error and produced zero false positives across the suite. That is the ground floor of trust. If a checker's verdict on the spec is unreliable, nothing else it does can be relied on either.

**What it is not:** a claim that Basilisk is automatically the best choice for your project. Conformance does not measure how fast a checker runs, how good its error messages are, how well it integrates with your editor, or how mature its ecosystem is. Those things matter enormously, and on some of them the older tools have years of head start.

Conformance is one input. It happens to be the input that decides whether you can trust the rest. But it is not the whole decision, and anyone who tells you a single number settles the question is selling something.

## Why we chased the number anyway

If conformance is not the whole story, why did we make 100% a hard requirement rather than a nice-to-have?

Because the alternative is a checker that is confidently wrong some of the time, and a checker that is confidently wrong is worse than no checker at all. The problem with Python typing was never the syntax. The problem was enforcement. A type hint that is never checked is a comment. A type hint that is checked by a tool with gaps is a comment that occasionally lies to you.

Basilisk's default rule set is the typing specification, with every spec rule on and nothing configured. There is no `--strict` flag to remember, because strict is the floor. When you run Basilisk on your code, the verdict you get is the one the specification says you should get. That is the entire point of the tool, and the conformance score is how we prove we actually did it rather than just claiming we did.

## How the score is produced, exactly

We measure this the boring, reproducible way, because that is the only kind of measurement worth publishing.

Basilisk's conformance number comes from the suite's own unmodified harness, run against the default-configuration Basilisk CLI, with every specification rule enabled and nothing special turned on. There is no vendored calculator and no home-grown scorer that could flatter the result. The harness that grades Basilisk is the same `python/typing` harness that grades Pyright, mypy, ty, Pyrefly, zuban, and pycroscope. If you clone the suite and run it yourself, you get the same board.

That is a deliberate design choice, and it maps to a rule we hold for everything we ship: self-measured metrics are only worth anything if they are reproducible and measured by a neutral party. The conformance suite is that neutral party. We just make sure our tool shows up and runs.

## Try it, and try to break it

You can see the full comparison, including how the score has moved over time, on our [conformance page](/docs/conformance/), and you can read the raw source of truth on the [python/typing results page]({{ conformanceOfficial.snapshot.source }}).

The best thing you can do, though, is point Basilisk at your own code and see where it disagrees with you. If it flags something the specification says is valid, that is a bug, and we want to hear about it on [GitHub](https://github.com/Nimblesite/Basilisk/issues). Basilisk got to {{ conformanceOfficial.basilisk.pct }}% by treating every reported gap as a real defect to fix, one at a time, against a referee that does not care how we feel about it. That is not going to change now that we are at the top of the board. If anything, it matters more.

Python's type system has been good enough to trust for a while. Now the tooling can be too.
