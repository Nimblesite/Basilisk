---
layout: layouts/blog.njk
title: "AI Agents Write Python Faster Than You Can Review It. Type Checking Is the Guardrail That Scales."
description: "Agentic coding tools generate Python at machine speed. A common and expensive failure, the hallucinated API, is exactly what a strict type checker catches before the code runs."
date: 2026-07-20
dateModified: 2026-07-20
author: The Basilisk Project
image: /assets/images/blog/ai-agents-python-type-checking.png
imageAlt: "An AI agent emitting Python that passes through a strict type-checking gate, valid code accepted and hallucinated code rejected"
imageWidth: 1200
imageHeight: 675
tags:
  - ai coding
  - type checking
category: deep-dives
excerpt: "Agentic coding went mainstream, and agents now write more Python than any human can carefully read. The good news: one of the most common ways agent code fails, calling functions that do not exist, is precisely the failure a type checker was built to catch. Here is why strict, default-on type checking is the guardrail that scales with the agents."
keywords: ai coding, agentic coding, google antigravity, ai code review, ai generated code bugs, python type checker, hallucinated api, llm code quality, static typing, ci guardrail, basilisk
faq:
  - q: "Can a type checker catch bugs in AI-generated code?"
    a: "It catches a specific and common class of them. Agentic tools frequently hallucinate APIs: they call functions that do not exist, invent method signatures, pass the wrong argument types, or access attributes that are not there. Every one of those is a type error, and a static type checker rejects them before the code runs. It does not catch logic errors, security vulnerabilities, or wrong business rules, which need other tools."
  - q: "What is the most common kind of bug in AI-generated code?"
    a: "Research on AI-generated code identifies functional bugs, including API and type errors, as the largest category, alongside a distinct hallucination category where the model produces syntactically valid code that references non-existent objects, functions, or libraries. Separately, one analysis of GitHub pull requests found AI-assisted PRs carried meaningfully more logic and correctness issues than human-only ones."
  - q: "Why does type checking scale better than code review for AI output?"
    a: "Human review does not scale to the volume agents produce, and reviewers tend to miss hallucinated APIs buried in large, plausible-looking diffs. A type checker runs deterministically on every diff, never gets tired, and rejects the whole hallucinated-API class automatically in CI before a person spends attention on it."
  - q: "Does strict-by-default matter for agent workflows?"
    a: "Yes. Most checkers hide strictness behind a flag or mode someone has to remember to enable, and an agent will not remember. A checker that enforces the full typing specification by default catches mistyped and hallucinated code out of the box, with no configuration the agent can forget to turn on."
---

Agentic coding stopped being a demo. This did not start with Google. Coding agents that plan, edit, and run code across a whole project, from Cursor to GitHub Copilot's agent mode, were already part of everyday developer workflows well before Google launched [Antigravity](https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform/) in November 2025, which it describes as "a new agentic development platform designed to help you operate at a higher, task-oriented level" that lets you "deploy agents that autonomously plan, execute, and verify complex tasks across your editor, terminal, and browser" ([Google Developers Blog](https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform/)). Antigravity is not the origin of this shift; it is a recent, high-profile marker of how far it has already gone. Every major editor now ships an agent that will happily write, run, and revise Python on your behalf.

That changes the shape of the problem. For most of the history of software, code was expensive to produce and the bottleneck was writing it. With agents, code is cheap and the bottleneck moves to trusting it. An agent can open twelve files and rewrite them before you have finished reading the first diff. So the question stops being "can we generate this" and becomes "how do we verify this, at the speed it arrives."

Here is the part that is good news for anyone who types their Python: one of the most common ways agent-written code fails is exactly the failure a type checker was built to catch.

## Agent code does not fail randomly

It is tempting to imagine AI bugs as exotic and unpredictable. In practice they cluster, and the clusters are well documented.

A recent academic survey, "A Survey of Bugs in AI-Generated Code," sorts the defects into categories and finds functional bugs, the group that includes API-related and type errors, to be the largest ([Gao, Tahir, Liang, Susnjak, and Khomh, arXiv](https://arxiv.org/html/2512.05239v1)). It also carves out a distinct **hallucination** category, where "the model may 'imagine' non-existent objects, functions, libraries, or behaviors," producing outputs "that appear syntactically correct and stylistically appropriate but are factually incorrect or inconsistent with real-world APIs" ([arXiv](https://arxiv.org/html/2512.05239v1)).

The industry data confirms the volume problem is real. One analysis of 470 open-source GitHub pull requests by the AI code-review company CodeRabbit found AI-generated pull requests carried roughly 1.7 times more issues on average than human-only ones ([as reported by ITBrief](https://itbrief.com.au/story/study-finds-ai-generated-code-far-buggier-than-human-work)). Worth being honest about: the sharpest jump in that analysis was in logic and correctness issues, up about 75%, and those are precisely the part a type checker will not save you from. What it does save you from is the other large and much cheaper slice, the hallucinated APIs and type mismatches.

If you have used these tools on real code, none of this will surprise you. The agent calls `response.json_body()` when the method is actually `response.json()`. It passes a `str` where the function wants an `int`. It accesses `user.email_address` on an object whose attribute is `email`. It imports a function from a module that moved it three releases ago. The code reads beautifully. It just is not true.

## A hallucinated API is a type error

Now line those failures up against what a static type checker actually does.

- Calls a function that does not exist. That is an unresolved name or an unknown attribute, and the checker flags it.
- Invents a method signature, passing the wrong number or type of arguments. That is an argument-type mismatch, and the checker flags it.
- Accesses an attribute the object does not have. That is an attribute error, caught statically instead of at 2am in production.
- Assumes the wrong return type and chains a method the real return value does not support. The checker follows the types and flags it.

Every one of those is a type error. A strict static type checker reads the code before it runs, resolves the names and types, and rejects the whole category deterministically. One of the agent's most common structural failures, confidently referencing something that is not there, is precisely the thing type checking exists to stop.

This is why the pairing is so natural. The agent is optimized to produce plausible code fast. A type checker is indifferent to plausibility; it only cares about whether the references and types actually line up. That is exactly the check a fast, confident generator needs sitting downstream of it.

## Review does not scale. Type checking does.

The usual answer to "how do we trust generated code" is "review it." That answer is under real strain.

Human review was calibrated for human output: a person writes a change, another person reads it. Agents break that ratio. They produce diffs faster and larger than reviewers can absorb, and the specific thing reviewers are worst at is catching a hallucinated API buried in an otherwise reasonable-looking change. Your eye slides right over `response.json_body()` because it looks like something that should exist. The mistake is invisible precisely because the surrounding code is fluent.

A type checker has none of that weakness. It does not get tired on the fortieth file. It does not assume a method exists because the name sounds right. It runs the same way on every diff, and it can run automatically in CI on every agent-generated pull request, rejecting the mistyped and hallucinated code before a human spends a minute of attention on it. Reviewers then get to focus on the things only humans can judge: is this the right design, does it match the intent, is the logic sound.

The economics are simple. Agents made code generation nearly free, so the value moved to verification. Type checking is verification that costs almost nothing per run and scales linearly with the volume of code, which is exactly the property you need when the volume is climbing fast.

## The honest boundary

We are going to be precise here, because this is where tooling marketing tends to overpromise.

**A type checker does not make AI code correct. It makes it well-typed.** Those are different guarantees, and the gap matters.

A type checker will not catch a logic error where the code is perfectly typed but computes the wrong answer. It will not catch a security vulnerability; Veracode's 2025 GenAI Code Security Report, which tested code from more than 100 models across dozens of tasks, found that 45% of AI-generated code samples introduced a security flaw, including OWASP Top 10 vulnerabilities, and closing those needs dedicated security tooling, not a type checker ([Veracode 2025 GenAI Code Security Report](https://www.veracode.com/blog/genai-code-security-report/)). It will not know that your business rule says refunds cap at 30 days. Those failures are real, and they need tests, security scanners, and human judgment.

What a type checker does is delete one large, specific, and extremely common category of agent bugs, the hallucinated-API-and-wrong-type class, cheaply and automatically. It is necessary, not sufficient. The right mental model is a guardrail, not a driver: it does not steer the car, it just keeps a whole class of mistakes from going off the road.

## Where Basilisk fits

Basilisk is an open-source, strict-by-default Python type checker and language server built in Rust. Two of its design choices matter specifically for agent workflows.

First, **strict is the default.** Most checkers hide their real strictness behind a flag or a mode that somebody has to remember to switch on, and an agent will never remember. Basilisk's default behavior is the full Python typing specification with every conformance rule on and no `--strict` to forget. That default is measurable, not a slogan: Basilisk scores 100% on the official [python/typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html), graded by the suite's own unmodified harness against its out-of-the-box configuration, the same harness that grades every other checker on that page. When generated code slips a wrong type through, the checker catches it without anyone having opted in.

Second, **it lives where the agent works.** Basilisk ships as a single binary with no runtime dependency, and one extension gives you the full language-server workflow in VS Code, Cursor, Zed, and Neovim: hover, go-to-definition, autocomplete, refactoring, integrated debugging, and profiling. The same engine runs in CI, so the check an agent's code faces in your editor is the identical check it faces on the pull request.

A note on scope, so you know exactly what you are getting. Basilisk checks types. It does not do security analysis, it does not detect logic errors, and it will not tell you whether the agent solved the right problem. It closes the type-error class, one of the classes agents blow most often, and it leaves the rest to the tools built for it.

## The bottom line

Agentic coding is genuinely useful, and it is not going back in the box. But cheap generation moved the whole game to verification, and verification has to run at machine speed to keep up with machine output. Human review will always matter for design and intent. It cannot, by itself, catch a confident agent inventing a function that does not exist, over and over, across more code than anyone can read.

That specific failure is a solved problem. A strict, default-on type checker rejects the hallucinated-API-and-wrong-type class automatically, on every diff, before the code runs. Of all the guardrails you can put around an agent writing Python, it is one of the cheapest and one of the most certain.

If you want type checking that is strict by default instead of a flag an agent will forget, [give Basilisk a try](https://github.com/Nimblesite/Basilisk).

## Frequently asked questions

### Can a type checker catch bugs in AI-generated code?

It catches a specific and common class of them. Agentic tools frequently hallucinate APIs: they call functions that do not exist, invent method signatures, pass the wrong argument types, or access attributes that are not there. Every one of those is a type error, and a static type checker rejects them before the code runs. It does not catch logic errors, security vulnerabilities, or wrong business rules, which need other tools.

### What is the most common kind of bug in AI-generated code?

Research on AI-generated code identifies functional bugs, including API and type errors, as the largest category, alongside a distinct hallucination category where the model produces syntactically valid code that references non-existent objects, functions, or libraries ([A Survey of Bugs in AI-Generated Code, arXiv](https://arxiv.org/html/2512.05239v1)). Separately, an analysis of 470 pull requests by CodeRabbit found AI-generated PRs carried meaningfully more logic and correctness issues than human-only ones, which is a reminder that type checking addresses one slice of the problem, not all of it ([as reported by ITBrief](https://itbrief.com.au/story/study-finds-ai-generated-code-far-buggier-than-human-work)).

### Why does type checking scale better than code review for AI output?

Human review does not scale to the volume agents produce, and reviewers tend to miss hallucinated APIs buried in large, plausible-looking diffs. A type checker runs deterministically on every diff, never gets tired, and rejects the whole hallucinated-API class automatically in CI before a person spends attention on it.

### Does strict-by-default matter for agent workflows?

Yes. Most checkers hide strictness behind a flag or mode someone has to remember to enable, and an agent will not remember. A checker that enforces the full typing specification by default catches mistyped and hallucinated code out of the box, with no configuration the agent can forget to turn on.

### Does Basilisk detect security issues in AI-generated code?

No. Basilisk checks types. Security vulnerabilities, which independent analyses find in a substantial share of AI-generated code, need dedicated security tooling. Basilisk closes the type-error class, one of the failures agents produce most often, and leaves security and logic to the tools built for those jobs.
