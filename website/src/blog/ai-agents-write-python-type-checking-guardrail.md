---
layout: layouts/blog.njk
title: "Python Type Checking Is the Guardrail AI Coding Agents Need"
description: "AI coding agents make repository-wide Python changes. Learn how strict type checking can catch typed API mistakes, where Any escapes, and what Basilisk checks."
date: 2026-07-20
dateModified: 2026-07-21
author: The Basilisk Project
image: /assets/images/blog/ai-agents-python-type-checking.png
imageAlt: "An AI agent emitting Python that passes through a strict type-checking gate, valid code accepted and hallucinated code rejected"
imageWidth: 1200
imageHeight: 675
tags:
  - ai coding
  - type checking
category: deep-dives
excerpt: "Claude Code and Codex can edit and test across a repository. With usable static types, a checker can catch nonexistent members, incompatible arguments, and unsupported calls before runtime. Here is where that guardrail works, where Python's Any escapes it, and what Basilisk checks today."
keywords: ai coding agents, agentic coding, claude code, openai codex, ai code review, ai generated code bugs, python type checking, python type checker, hallucinated api, llm code quality, static typing, ci guardrail, basilisk
faq:
  - q: "Can a type checker catch bugs in AI-generated code?"
    a: "It can catch the inconsistencies it can prove from static types. With typed project code or library stubs, that includes many nonexistent members, incompatible arguments, and invalid call shapes. Values typed as Any, untyped dependencies, dynamic attributes, logic errors, security vulnerabilities, and wrong business rules remain outside that guarantee."
  - q: "What is the most common kind of bug in AI-generated code?"
    a: "A 2025 survey found functional bugs were the most commonly discussed broad category across the studies it reviewed. Logic and semantic bugs were prominent within that category. The paper also documented hallucinated identifiers and methods, but said research on explicitly labelled hallucination bugs remained limited."
  - q: "Where does type checking help in an AI code-review workflow?"
    a: "It automates repeatable checks against known interfaces before human review. A checker can flag provable type and API mismatches on every diff, leaving reviewers to concentrate on design, intent, logic, and security. It complements review; it does not replace it."
  - q: "Does strict-by-default matter for agent workflows?"
    a: "It removes a configuration step from automated checks. Basilisk has no separate strict mode: its check command runs all PEP-tagged rules by default, while additional house rules are configured separately and seeded for new LSP workspaces. Precise results still depend on the static type information available."
  - q: "Does Basilisk detect security issues in AI-generated code?"
    a: "No. Basilisk checks types; it is not a security analyser. Security vulnerabilities need dedicated security tooling, while logic and business-rule errors need tests and human judgment."
---

In February 2025, Anthropic introduced [Claude Code](https://www.anthropic.com/news/claude-3-7-sonnet), a terminal-based agent that could search and read code, edit files, write and run tests, and use command-line tools. In May, OpenAI introduced [Codex](https://openai.com/index/introducing-codex/), a cloud software-engineering agent that could read and edit repositories, run commands and tests, and handle multiple tasks in parallel. Both go beyond autocomplete: they accept repository-level tasks and return code changes.

That changes the verification problem. When a tool can produce a plausible multi-file patch quickly, manual review should not be the only control. [OpenAI's own launch guidance](https://openai.com/index/introducing-codex/) says agent-generated code still needs manual review and validation. The practical question is which deterministic checks can run alongside that review.

For typed Python, static type checking is one such check. When usable type information is available, it can catch nonexistent members, incompatible arguments, and invalid call shapes before execution. That is narrower than saying a type checker catches every hallucinated API, but it is also a guarantee you can actually build a workflow around.

## Agent code does not fail randomly

AI-generated defects cluster into recognisable categories. A 2025 academic survey, *A Survey of Bugs in AI-Generated Code*, found functional bugs were the most commonly discussed broad category: 56 studies, or 78% of those reviewed, covered them. Within that category, variable, object, parameter, and property bugs accounted for 27% of the catalogued cases, while semantic and logic bugs accounted for 26% ([Gao, Tahir, Liang, Susnjak, and Khomh, arXiv](https://arxiv.org/html/2512.05239v1)).

The same paper documents hallucination as a distinct, overlapping failure mode: generated code can refer to fabricated variables, objects, methods, or libraries while remaining syntactically plausible. It also says research on bugs explicitly labelled as hallucinations is still limited. The evidence supports calling hallucinated APIs a documented failure mode, not the most common one.

A separate [CodeRabbit vendor analysis](https://www.coderabbit.ai/blog/state-of-ai-vs-human-code-generation-report) compared 320 pull requests it classified as AI-coauthored with 150 it treated as human-only. It reported about 1.7 times as many issues per AI-coauthored pull request and 75% more logic and correctness findings. CodeRabbit also states the study's key limitation: it inferred authorship from repository signals and could not verify every classification. Treat those results as directional evidence from a code-review vendor, not proof that AI caused every difference.

The failure patterns are familiar. An agent writes `json.parse_body()` when the module exposes `json.loads()`. It passes a string literal where a known function signature requires an integer. It imports a name that moved between releases. It accesses an attribute that sounds right but is absent from the declared interface. The code can look fluent while disagreeing with the API it is supposed to use.

## When a hallucinated API becomes a type error

Many hallucinated APIs become type errors when the checker has a precise static model of the code through annotations, source it can analyse, or `.pyi` stubs.

- If the callee's signature is known, a checker can reject an incompatible argument or call shape.
- If a module or object's member surface is statically known, a checker can reject an undeclared member.
- If a return type is known and propagated, a checker can reject a chained operation that type does not support.
- If an import cannot be resolved statically, a checker can report the missing import or loss of type information.

For example, Typeshed describes the public surface of Python's standard library, so an invented `json.parse_body` member is statically testable:

```python
import json

payload = json.parse_body('{"ready": true}')
```

That condition matters. Python has a [gradual type system](https://typing.python.org/en/latest/spec/concepts.html#gradual-types): an expression typed as `Any` is deliberately outside ordinary static checking. If `client` is `Any`, then `client.parse_body()` is allowed by the typing rules because the checker has no static member surface to compare it with. Untyped dependencies, dynamic `getattr`, monkey-patching, and inference gaps can therefore hide an invented API.

Type checking catches the failures it can prove from the available types. It does not eliminate the entire hallucinated-API class.

## Review needs automated checks beside it

Human review remains essential, but it is a poor place to repeat mechanical interface checks. A plausible method name can pass a quick visual read because it looks like something the library might provide. A type checker does not judge plausibility; it compares the operation with the static information it has.

That makes the division of labour useful. Run type checking automatically in the editor and CI, and let it report provable type incompatibilities on every diff. Reviewers can then concentrate on the questions static types cannot answer: is this the right design, does it match the intent, is the logic sound, and is the change safe?

The advantage is repeatability, not magic. The checker applies the same rules to each change, but the quality of its answer depends on annotations, stubs, inference, configuration, and the checker's implemented coverage.

## The honest boundary

**A type checker does not make AI code correct. It rejects inconsistencies it can prove from the program's static types.**

It will not catch a logic error where the types line up but the answer is wrong. It is not a substitute for security analysis: in Veracode's 2025 benchmark of more than 100 language models across Java, Python, C#, and JavaScript, 45% of generated samples failed its security tests and introduced an OWASP Top 10 vulnerability ([Veracode 2025 GenAI Code Security Report](https://www.veracode.com/blog/genai-code-security-report/)). That result describes Veracode's benchmark, not every piece of AI-generated code, and closing those failures needs dedicated security tooling. A checker also cannot know that your business rule caps refunds at 30 days.

The right mental model is a guardrail, not a driver. Static typing can remove a useful set of high-confidence interface mistakes before runtime. Tests, security scanners, and human judgment still have to cover the rest.

## Where Basilisk fits

Basilisk is an open-source Python type checker and language server built in Rust. Two design choices matter for agent workflows.

First, **there is no separate `--strict` mode to remember.** [`basilisk check`](/docs/configuration/) runs all of Basilisk's PEP-tagged rules by default. Additional house rules are configured separately, and a new LSP workspace seeds those rules at error severity. The current official [python/typing conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html) list Basilisk 0.27.0 as passing all 141 test files. That score describes the published conformance fixtures; it is not a promise that every possible Python type error or hallucinated API will be detected.

Second, **the checker can run where the agent works.** Basilisk's checker and language server share one native Rust process, with [integrations for VS Code and Cursor, Zed, and Neovim](/docs/installation/). The editor and CLI use the same parser-resolver-checker pipeline, so matching configuration, type sources, Python target, and diagnostic scope produces the same type-checking result. Optional workflows can invoke external components, including Python and `debugpy` for [debugging](/docs/debugging/) and a helper for [profiling](/docs/profiler/).

Basilisk's current implementation can report undeclared attributes on modules backed by Typeshed or authoritative local stubs and incompatible literal arguments for known signatures; see its [module-attribute](/errors/imports_module_attribute/) and [argument-type](/errors/calls_argument_type/) diagnostics. It remains conservative around `Any`, untyped inline third-party packages, ordinary dynamic instance attributes, and inference gaps. In other words, Basilisk catches the type errors it can prove from available static information. It does not claim to catch every hallucinated API.

## The bottom line

Claude Code and Codex make repository-wide changes, so verification needs to sit inside the agent workflow rather than arrive as an afterthought. Type checking is one deterministic layer in that workflow: it can reject operations that contradict known interfaces before the code runs or reaches review.

That is not the whole hallucinated-API class. It is the statically visible subset, and its size depends on how well typed the project and its dependencies are. It is still valuable because the check is repeatable, automatable, and leaves human attention for logic, security, design, and intent.

If that boundary matches what you want from a Python type checker, [install Basilisk](/docs/installation/) and run it in the editor and CI.

## Frequently asked questions

### Can a type checker catch bugs in AI-generated code?

It can catch the inconsistencies it can prove from static types. With typed project code or library stubs, that includes many nonexistent members, incompatible arguments, and invalid call shapes. Values typed as `Any`, untyped dependencies, dynamic attributes, logic errors, security vulnerabilities, and wrong business rules remain outside that guarantee. Python's typing specification explains why operations on [`Any` cannot be checked statically](https://typing.python.org/en/latest/spec/concepts.html#gradual-types).

### What is the most common kind of bug in AI-generated code?

A 2025 survey found functional bugs were the most commonly discussed broad category across the studies it reviewed. Logic and semantic bugs were prominent within that category. The paper also documented hallucinated identifiers and methods, but said research on explicitly labelled hallucination bugs remained limited ([*A Survey of Bugs in AI-Generated Code*, arXiv](https://arxiv.org/html/2512.05239v1)).

### Where does type checking help in an AI code-review workflow?

It automates repeatable checks against known interfaces before human review. A checker can flag provable type and API mismatches on every diff, leaving reviewers to concentrate on design, intent, logic, and security. It complements review; it does not replace it.

### Does strict-by-default matter for agent workflows?

It removes a configuration step from automated checks. Basilisk has no separate strict mode: its `check` command runs all PEP-tagged rules by default, while additional house rules are configured separately and seeded for new LSP workspaces. Precise results still depend on the static type information available.

### Does Basilisk detect security issues in AI-generated code?

No. Basilisk checks types; it is not a security analyser. Security vulnerabilities need dedicated security tooling, while logic and business-rule errors need tests and human judgment.
