---
layout: layouts/blog.njk
title: "OpenAI Acquires Astral: What It Means for Basilisk"
description: "OpenAI is acquiring Astral, the makers of Ruff, uv, and ty. Basilisk is built on Ruff. Here is exactly what changes, what does not, and our commitment to keeping it open."
date: 2026-06-20
author: The Basilisk Project
tags: posts
category: announcements
# No Chinese translation of this post exists yet — opt out of the language
# switcher so it doesn't link to a 404 at /zh/blog/.../ (see base.njk).
noTranslation: true
excerpt: "Basilisk's parser is Ruff. Ruff's owner is about to become OpenAI. Here is what that means — backed by the primary sources, quote by quote — and why your Basilisk install is safe regardless."
keywords: basilisk, astral, openai, ruff, uv, ty, charlie marsh, python type checker, open source, fork, mit license
---

On **March 19, 2026**, Astral — the company behind Ruff, uv, and the ty type checker — announced it had "entered into an agreement to join OpenAI as part of the Codex team" ([astral.sh/blog/openai](https://astral.sh/blog/openai); [openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)).

Basilisk is built directly on Astral's work. So this is not abstract industry news for us — it touches the foundation of the product. This post lays out exactly what happened, what the people in charge actually said, what it changes for Basilisk, and what it does not. Every claim below links to its source. Where a quote mattered, we verified it against a second source before printing it.

## What actually happened

- OpenAI is acquiring **Astral**, the maker of the open-source Python tools **Ruff** (linter/formatter), **uv** (package/project manager), and **ty** (type checker). The Astral team joins OpenAI's **Codex** team after close ([openai.com](https://openai.com/index/openai-to-acquire-astral/), [astral.sh](https://astral.sh/blog/openai)).
- The deal **is not closed**. It is "still subject to customary closing conditions, including regulatory approval," and "until then, OpenAI and Astral will remain separate, independent companies" (OpenAI's announcement, [openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)).
- OpenAI's stated intent post-close: "after closing OpenAI plans to support Astral's open source products," and "over time, OpenAI will explore deeper integrations that allow Codex to interact more directly with the tools developers already use" ([openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)).
- OpenAI frames the rationale plainly: "By bringing Astral's tooling and engineering expertise to OpenAI, we will accelerate our work on Codex" (OpenAI, as quoted in [Simon Willison's write-up](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/)).

## Give the man his props: Charlie Marsh, on the record

Charlie Marsh built Ruff and uv into the de facto standard of modern Python tooling in roughly three years. That is one of the great runs in developer-tooling history, and it is worth saying out loud. Here is what he wrote when announcing the deal ([astral.sh/blog/openai](https://astral.sh/blog/openai)):

> "I started Astral to make programming more productive."
>
> "Today, we're taking a step forward in that mission by announcing that we've entered into an agreement to join OpenAI as part of the Codex team."

On open source — the line that matters most to anyone downstream of Astral, and the one we independently confirmed against a second source:

> "Open source is at the heart of that impact and the heart of that story; it sits at the center of everything we do."
>
> "OpenAI will continue supporting our open source tools after the deal closes. We'll keep building in the open, alongside our community."

That is the commitment, in the founder's own words. Credit where it's due: Marsh didn't hedge it, and OpenAI's own announcement says the same thing in its own voice (above). **Props to Charlie Marsh** — Basilisk exists in part because his team made a world-class Python parser available under a permissive license, and we don't intend to let that go unacknowledged.

## The bedrock: the licenses (verified, not assumed)

Reassurances are nice. **Licenses are binding.** This is the single most important fact in the whole story, so we verified each one directly against the source repositories rather than trusting a summary:

| Tool | License | Source |
|---|---|---|
| **Ruff** | MIT — "MIT License. Copyright (c) 2022 Charles Marsh" | [astral-sh/ruff/LICENSE](https://github.com/astral-sh/ruff/blob/main/LICENSE) |
| **uv** | Dual: "licensed under either of Apache License, Version 2.0 or MIT license at your option" | [astral-sh/uv](https://github.com/astral-sh/uv#license) |
| **ty** | MIT — "ty is licensed under the MIT license" | [astral-sh/ty](https://github.com/astral-sh/ty#license) |

Permissive licenses are irrevocable for code already published. No acquirer — not OpenAI, not anyone — can retroactively un-license the Ruff that exists today. The grant has already been made. The worst a future owner can do is change the license on *future* commits, at which point the community keeps the last open commit and continues from there. As Astral's Douglas Creager put it, the permissive licensing "makes the worst-case scenarios have the shape of 'fork and move on'" (via [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/)).

## How the rest of the ecosystem read it

We're not the only downstream project doing this math.

- **Armin Ronacher** (creator of Flask) called uv "a very forkable and maintainable thing," arguing the community would be better off than before uv existed even in a worst case (via [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/); JetBrains echoes the same "very forkable and maintainable" framing in [their post](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/)).
- **Simon Willison**: "I like and trust the Astral team and I'm optimistic that their projects will be well-maintained in their new home" ([simonwillison.net](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/)).
- **JetBrains** (PyCharm), a direct integrator of both Ruff and uv: "We've integrated both Ruff and uv into PyCharm because they substantially make Python development better… We will continue to do so." They add the key point for everyone downstream: "Astral's tools are open-source under permissive licenses. The community can fork them if it ever comes to that," and "Regardless of who owns these tools, our commitment to supporting the best Python tooling for our users stays the same" ([blog.jetbrains.com](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/)).
- The **Python community discussion** ranged from cautious optimism to genuine concern about a foundational tool being owned by a frontier AI lab. It's worth reading in full and forming your own view: [discuss.python.org/t/openai-to-acquire-astral](https://discuss.python.org/t/openai-to-acquire-astral/106605).

## What this means for Basilisk — specifically

Basilisk's relationship to Astral is concrete and load-bearing:

1. **Our parser is Ruff's parser.** Basilisk depends on `ruff_python_parser`, `ruff_python_ast`, and `ruff_text_size`, pinned to an **immutable git commit** (`rev 7c645a9`, equal to tag `0.15.17`) on `astral-sh/ruff`. We pin a `rev`, not a tag, precisely so the version "can never be swapped out from under us." That code is MIT-licensed and already in our `Cargo.lock`. Nothing about this acquisition can reach back and change the bytes we build against.
2. **Our lint/format path shells out to the Ruff CLI** — `ruff==0.15.17`, pinned identically in CI and the dev container. Same story: a pinned, permissively licensed binary we control the version of.
3. **ty is now an OpenAI-backed competitor.** Astral's type checker, ty, occupies the same conceptual space as the Basilisk checker, and it will now have OpenAI's resources behind it. We take that seriously — but it sharpens, rather than threatens, what makes Basilisk different: **strict-by-default conformance**, one **complete LSP** (test explorer, debugging, profiling, autofixes) in a single extension, and a relentless march toward 100% PEP conformance. A faster-funded type checker validates the bet that Python deserves first-class, Rust-speed tooling — it doesn't make our differentiation any less true.
4. **The architecture bet is shared — and now vindicated.** Basilisk, like Astral's tools, is built in Rust on the Ruff AST with Salsa for incrementality. Astral proved that stack scales to millions of users. We made the same call independently. That's reassuring, not threatening.

**Net effect on you, today:** zero. Your Basilisk install builds from pinned, MIT-licensed Ruff code and a pinned Ruff binary. The acquisition does not, and cannot, alter either.

## Nimblesite's commitment

Let us be unambiguous, because our users deserve a straight answer:

**Nimblesite is committed to keeping Basilisk's foundation open source — and we will fork Ruff if it ever becomes necessary.**

The MIT license makes that not just possible but clean. We already pin Ruff to an immutable commit, which is a fork-in-waiting by design: if upstream's direction, license, or stewardship ever stopped serving Basilisk's users, we would carry the parser forward ourselves under its existing open-source terms. We hope never to need to. We are fully prepared to if we do. The freedom to walk away is exactly what permissive licensing buys you, and it is why we built on Ruff in the first place.

## Words of encouragement to OpenAI

We want to end on the right note, because the signals here are genuinely good and good behavior deserves to be named.

OpenAI didn't have to say anything about open source. It chose to — in its own announcement ("after closing OpenAI plans to support Astral's open source products," [openai.com](https://openai.com/index/openai-to-acquire-astral/)) and through Charlie Marsh ("OpenAI will continue supporting our open source tools after the deal closes. We'll keep building in the open, alongside our community," [astral.sh](https://astral.sh/blog/openai)). Those are commitments a company can be held to, and we intend to hold the whole ecosystem's memory of them.

To OpenAI: **thank you for keeping these tools open, and please keep them that way.** Ruff and uv became standards *because* they were open, fast, and community-built. Stewarding them generously — upstreaming improvements, keeping the parser reusable as a standalone crate, building in the open as promised — is not charity; it's the thing that made the asset worth acquiring. Keep faith with the license and the community, and you'll have ours. Basilisk is rooting for you to get this right, and we'll be one of the many downstream projects quietly proving, every day, why open foundations are worth protecting.

---

*Sources, in full: [OpenAI announcement](https://openai.com/index/openai-to-acquire-astral/) · [Astral's announcement (Charlie Marsh)](https://astral.sh/blog/openai) · [JetBrains / PyCharm](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/) · [Python Discourse thread](https://discuss.python.org/t/openai-to-acquire-astral/106605) · [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/) · Licenses: [Ruff (MIT)](https://github.com/astral-sh/ruff/blob/main/LICENSE), [uv (Apache-2.0 / MIT)](https://github.com/astral-sh/uv#license), [ty (MIT)](https://github.com/astral-sh/ty#license).*
