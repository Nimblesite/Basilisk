---
layout: layouts/blog.njk
title: "Free-Threaded Python: Why Type Checking Matters More Now"
description: "Python 3.14 makes free-threaded (no-GIL) Python official. Real parallelism means type errors cost more, and strict type checking is your cheapest defense."
date: 2026-07-11
author: The Basilisk Project
image: /assets/images/blog/free-threaded-python-type-checking.png
imageAlt: "Parallel Python execution threads running side by side over a strict type-checking safety layer"
imageWidth: 1200
imageHeight: 675
tags:
  - Python performance
  - Python typing
category: deep-dives
excerpt: "Python 3.14 made free-threaded Python officially supported. When threads really run in parallel, a type error stops being a quiet annoyance and becomes a concurrency bug. Here's why strict, default-on type checking is the cheapest protection you can buy."
keywords: free-threaded python, no-gil python, python 3.14, cpython, python type checker, python type hints, strict typing, concurrency, pep 703, pep 779, basilisk
faq:
  - q: "Is free-threaded Python officially supported?"
    a: "Yes. As of Python 3.14, released October 7, 2025, the free-threaded (no-GIL) build is officially supported under PEP 779, moving it out of the experimental status it had in Python 3.13."
  - q: "Does removing the GIL make my existing code unsafe?"
    a: "Removing the GIL does not change your code, but it removes the serialization that often kept concurrency bugs from surfacing. The official free-threading HOWTO recommends using threading.Lock or other synchronization primitives for shared state rather than relying on the interpreter's internal locks."
  - q: "Can a type checker detect data races?"
    a: "No. A static type checker, including Basilisk, does not detect data races or analyze locking. What it does is eliminate type errors, which is a bug category that becomes more dangerous under real parallelism. Static typing and runtime synchronization primitives solve different problems, and under free-threading you want both."
  - q: "What is the performance cost of free-threaded Python?"
    a: "According to the Python 3.14 release notes, the single-threaded performance penalty in free-threaded mode is now roughly 5-10%, depending on the platform and C compiler used, a significant improvement over earlier builds."
  - q: "How does Basilisk help with all of this?"
    a: "Basilisk enables its typing-spec rules by default, with no strict flag to remember. Its former conformance result has been withdrawn, however, and its actual percentage is temporarily unknown while affected logic is reimplemented and verified."
---

Free-threaded Python stopped being an experiment. As of Python 3.14, released on October 7, 2025, the free-threaded (no-GIL) build is officially supported, not experimental, under [PEP 779](https://peps.python.org/pep-0779/) ([Python 3.14 release notes, python.org](https://docs.python.org/3/whatsnew/3.14.html)). If you have been half-watching the "no-GIL" story for the last few years, this is the moment it went real.

We think this is one of the biggest changes to Python since type hints landed, and it has a direct consequence that not enough people are talking about: **when your threads actually run in parallel, the cost of a type error goes up.** So let's walk through what changed, why it raises the stakes, and what you can do about it today.

## What actually changed in Python 3.14

For most of Python's life, the Global Interpreter Lock (the GIL) meant that only one thread executed Python bytecode at a time. You could write threaded code, but the GIL quietly serialized it. That made a whole class of concurrency bugs rare in practice, because two threads almost never touched the same object at the exact same instant.

[PEP 703](https://peps.python.org/pep-0703/), accepted by the Python Steering Council on October 24, 2023, proposed making the GIL optional through a `--disable-gil` build. Python 3.13 shipped that build as experimental. Python 3.14 finished the job: per the [official 3.14 release notes](https://docs.python.org/3/whatsnew/3.14.html), "the implementation described in PEP 703 has been finished, including C API changes," and the single-threaded performance penalty is now "roughly 5-10%, depending on the platform and C compiler used."

The rollout follows three phases, described in [PEP 779](https://peps.python.org/pep-0779/):

- **Phase I (Python 3.13):** free-threaded build available, but explicitly experimental.
- **Phase II (Python 3.14):** free-threaded build officially supported, but optional.
- **Phase III (future):** free-threaded build becomes the default.

We are in Phase II right now. That "officially supported" label is the part that changes how you should think about your code.

## Why real parallelism raises the stakes on type errors

Here is the uncomfortable part. When threads genuinely run at the same time, two of them can read and write the same object concurrently. The official [free-threading HOWTO](https://docs.python.org/3/howto/free-threading-python.html) is careful about what the interpreter guarantees for you and what it does not:

> "Python has not historically guaranteed specific behavior for concurrent modifications to these built-in types, so this should be treated as a description of the current implementation, not a guarantee of current or future behavior."

And its recommendation is blunt:

> "It's recommended to use the `threading.Lock` or other synchronization primitives instead of relying on the internal locks of built-in types, when possible."

So correctness under real parallelism is your job, not the interpreter's. Now think about what a type error does in that world. Suppose one function believes a shared value is a `list` and another treats it as a `dict`. Under the old GIL, that mismatch might have surfaced as an occasional, reproducible exception. Under free-threading, the same mismatch can now interleave with a second thread's access and turn into something far worse: a corrupted structure, a silent wrong answer, or a crash that only shows up under load and never reproduces on your laptop.

The type error was always a bug. Parallelism just removed the padding that used to hide it. A wrong type flowing through your program is exactly the kind of defect that gets more dangerous, not less, the moment the GIL stops serializing everything for you.

We want to be precise with you here, because this is where a lot of tooling marketing gets sloppy. **A type checker does not detect data races.** Basilisk does not have threading rules, race detection, or a lock analyzer, and we are not going to pretend it does. What a type checker does is eliminate the whole category of "this value is not the type this code assumes it is" before your program ever runs. In a free-threaded world, that category is where a surprising number of your worst, least reproducible bugs will come from. Closing it up front is cheap. Debugging a heisenbug in production is not.

## Type hints have been optional for ten years. That's the deeper problem.

Type hints arrived with [PEP 484](https://peps.python.org/pep-0484/) in Python 3.5, back in 2015. Ten years on, most Python developers write them. But writing a type hint and enforcing it are two very different things, and the enforcement half is where teams quietly fall down.

Most Python type checkers treat strictness as opt-in. There is a flag, or a mode, or a config setting you were supposed to turn on, and in a lot of real codebases nobody ever did. So you end up with annotations that look like a safety net but catch nothing, because the checker is running in a mode that lets untyped and mistyped code slide through.

That was already a problem when the GIL was covering for you. In a free-threaded world it is a bigger one, because the bugs those annotations were supposed to catch now have sharper teeth.

## What you can do today

You do not need to wait for Phase III or rewrite anything to get ahead of this. A few concrete moves:

1. **Turn type checking on by default, not as an optional flag.** The single most common failure we see is a project with beautiful type hints and a checker running in a permissive mode that ignores most of them. If strictness is something a teammate has to remember to enable, it will get forgotten.
2. **Check types in CI, on every pull request.** Roughly 30% of developers who use type hints "always" or "often" still have no type checking in their CI pipeline at all ([Typed Python 2024 developer survey, Meta and Microsoft](https://engineering.fb.com/2024/12/09/developer-tools/typed-python-2024-survey-meta/)). A hint that is never checked is a comment.
3. **Follow the free-threading guidance for genuinely shared state.** Use `threading.Lock` or another synchronization primitive rather than leaning on the interpreter's internal locks, exactly as the [official HOWTO](https://docs.python.org/3/howto/free-threading-python.html) recommends. Static typing and real locks solve different problems, and under free-threading you want both.

## Where Basilisk fits

Basilisk is our answer to the "enforcement is optional" problem. It is an open-source Python type checker and language server built in Rust, with its typing-spec rules enabled and no `--strict` flag to forget.

**Correction:** Basilisk's former conformance result is withdrawn. Test-specific implementation logic made that number untrustworthy, Basilisk has been removed from the official results at our request, and its current percentage is temporarily unknown. See the [conformance correction](/docs/conformance/) for the clean reimplementation and robustness-testing work now underway.

A few honest boundaries so you know exactly what you are getting:

- Basilisk has no canonical Python target. It applies version-dependent behavior only where the maintained typing specification, an accepted PEP, or Python syntax requires it ([pinned typing directives, `python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)). The type-safety argument here holds regardless of which supported interpreter the project selects.
- Basilisk has **no concurrency-specific analysis.** Its job is catching type errors, which is the general-purpose defense that gets more valuable once the GIL is no longer serializing your program for you.
- Beyond the default PEP-derived rule set, a small set of stricter house-style rules are one config change away when you want them: require a type on every parameter (`BSK-0001`) and every return (`BSK-0002`), require `@override` when you override a base method (`BSK-0025`), flag redundant annotations (`BSK-0050`), and nudge on explicit `Any` (`BSK-0014`). They are off by default and scoped per project.

It ships as a single binary with no runtime dependency, and one extension gives you the full workflow in VS Code, Cursor, Zed, and Neovim: hover, go-to-definition, autocomplete, refactoring, integrated debugging, and profiling.

## The bottom line

Free-threaded Python is a genuinely good thing. Real parallelism is something the language has wanted for a long time, and the 3.14 team earned this milestone. But every powerful feature comes with a bill, and the bill for real parallelism is correctness discipline. The interpreter has told you plainly that concurrent correctness is now partly your responsibility.

Static type checking will not write your locks for you. What it will do is delete an entire class of bugs before they ever get the chance to interleave badly across two threads at 3am in production. That was always worth doing. In a free-threaded world, it is one of the cheapest insurance policies you can buy.

If you want strictness that is on by default instead of a flag you have to remember, [give Basilisk a try](https://github.com/Nimblesite/Basilisk).

## Frequently asked questions

### Is free-threaded Python officially supported?

Yes. As of Python 3.14, released October 7, 2025, the free-threaded (no-GIL) build is officially supported under [PEP 779](https://peps.python.org/pep-0779/), moving it out of the experimental status it had in Python 3.13 ([Python 3.14 release notes](https://docs.python.org/3/whatsnew/3.14.html)).

### Does removing the GIL make my existing code unsafe?

Removing the GIL does not change your code, but it removes the serialization that often kept concurrency bugs from surfacing. The [official free-threading HOWTO](https://docs.python.org/3/howto/free-threading-python.html) recommends using `threading.Lock` or other synchronization primitives for shared state rather than relying on the interpreter's internal locks, and it notes that concurrent-modification behavior for built-in types is "not a guarantee of current or future behavior."

### Can a type checker detect data races?

No. A static type checker, including Basilisk, does not detect data races or analyze locking. What it does is eliminate type errors, which is a bug category that becomes more dangerous under real parallelism. Static typing and runtime synchronization primitives solve different problems, and under free-threading you want both.

### What is the performance cost of free-threaded Python?

According to the [Python 3.14 release notes](https://docs.python.org/3/whatsnew/3.14.html), the single-threaded performance penalty in free-threaded mode is now "roughly 5-10%, depending on the platform and C compiler used," a significant improvement over earlier builds.

### How does Basilisk help with all of this?

Basilisk is a Python type checker whose typing-spec rules are enabled by default, with no strict flag to forget. Its former conformance result is withdrawn and its current percentage is temporarily unknown while affected logic is rebuilt and verified; evaluate it against your own code rather than relying on the old figure.
