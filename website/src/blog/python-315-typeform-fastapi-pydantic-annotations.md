---
layout: layouts/blog.njk
title: "Python 3.15: The Type Hints FastAPI and Pydantic Actually Run"
description: "Python 3.15 rc1 ships TypeForm, closed TypedDicts, and disjoint bases. Here is what the three new typing PEPs change for FastAPI and Pydantic code."
date: 2026-08-04
dateModified: 2026-08-06
author: The Basilisk Project
image: /assets/images/blog/python-315-annotations-fastapi-pydantic.png
imageAlt: "A translucent specification plate is scanned in cyan as it activates an orange-lit precision engine"
imageWidth: 1200
imageHeight: 675
tags:
  - FastAPI
  - Pydantic
  - Python typing
category: deep-dives
excerpt: "Python 3.15 reaches its first release candidate today with three typing PEPs finalised. All three matter most in the code where annotations are not documentation — they are the program."
keywords: Python 3.15, TypeForm, PEP 747, PEP 728, PEP 800, closed TypedDict, extra_items, disjoint base, Python annotations, Python type hints, FastAPI type hints, Pydantic TypeAdapter, Python type checker, Basilisk
faq:
  - q: "What typing features are new in Python 3.15?"
    a: "Python 3.15 implements three typing PEPs: PEP 747 adds typing.TypeForm for annotating values that are themselves type expressions, PEP 728 adds the closed and extra_items class arguments to TypedDict, and PEP 800 adds the typing.disjoint_base decorator. All three are marked Final with Python-Version 3.15."
  - q: "What is TypeForm in Python?"
    a: "TypeForm is a special form added by PEP 747. TypeForm[T] describes the set of all type form objects that represent the type T or types assignable to T. It lets a function take a type expression such as list[int] or int | None as a runtime argument while still telling a type checker what the function returns."
  - q: "Why does TypeForm matter for Pydantic and FastAPI?"
    a: "Functions like pydantic.TypeAdapter accept a type expression as a runtime value. PEP 747 names TypeAdapter(T).validate_python among its motivating examples. Before TypeForm there was no accurate way to annotate that parameter, because type[T] only covers classes and not forms like list[int], int | None, or Literal values."
  - q: "What does closed=True do on a TypedDict?"
    a: "PEP 728 adds closed and extra_items class arguments to TypedDict. A closed TypedDict does not allow extra keys beyond those declared in the class body, while extra_items allows arbitrary extra items whose values are of the specified type."
  - q: "Does Basilisk support the Python 3.15 typing features?"
    a: "Basilisk's support status for these features is being revalidated. The previously published pass counts and overall conformance result are withdrawn because test-specific implementation logic made the result untrustworthy."
---

Python 3.15 reaches its first release candidate today. [PEP 790](https://peps.python.org/pep-0790/), the 3.15 release schedule, puts rc1 on 2026-08-04 and the final release on 2026-10-01. The feature set is frozen; what is in the tree now is what ships in October.

Three typing PEPs made it in. Per the [Python 3.15 release notes](https://docs.python.org/3.15/whatsnew/3.15.html), 3.15 adds [`typing.TypeForm`](https://peps.python.org/pep-0747/), the `closed` and `extra_items` class arguments on [`TypedDict`](https://peps.python.org/pep-0728/), and the [`@typing.disjoint_base`](https://peps.python.org/pep-0800/) decorator. All three PEPs are marked Final with Python-Version 3.15.

If you write ordinary application code, these land as three modest conveniences. If you write FastAPI or Pydantic code, they land somewhere different — because in that code your annotations are not commentary on the program. They *are* the program.

## Annotations as executable code

Most Python type hints are inert at runtime. In a FastAPI application they are not.

FastAPI's own documentation is direct about this: "**FastAPI** is all based on these type hints" and "**FastAPI** is all based on Pydantic." It goes on to list what the framework does with the annotation you wrote — it uses the same declarations to "**Define requirements**: from request path parameters, query parameters, headers, bodies, dependencies," to "**Convert data**: from the request to the required type," to "**Validate data**," and to "**Document** the API using OpenAPI" ([FastAPI, *Python Types Intro*](https://fastapi.tiangolo.com/python-types/)).

That is four distinct runtime behaviours derived from one annotation. Change `int` to `str` in a route signature and you have changed the parser, the validator, the error responses your clients receive, and your published OpenAPI schema — in one keystroke, with no call site to grep for.

This is why the annotation-heavy frameworks feel qualitatively different to work in. A wrong type hint in a plain library is a documentation defect. A wrong type hint in a FastAPI route is a production defect, and it ships silently, because the code still runs.

Python 3.14 made this style structurally cheaper. [PEP 649](https://peps.python.org/pep-0649/) and [PEP 749](https://peps.python.org/pep-0749/), both Final for 3.14, moved annotations to lazy evaluation and added the `annotationlib` module, whose aim the PEP describes as "to provide tooling for introspecting and wrapping annotations." Annotations became something libraries can inspect deliberately rather than something they must race to evaluate at import time.

Python 3.15's contribution is the next step: making the annotations themselves capable of saying more.

## PEP 747: the type expression as a value

Start with the pattern that has been unannotatable for years:

```python
from pydantic import TypeAdapter

TypeAdapter(list[User])
TypeAdapter(int | None)
TypeAdapter(Literal["draft", "published"])
```

Pydantic describes `TypeAdapter` as something that "can be used for type validation, serialization, and JSON schema generation without needing to create a `BaseModel`," capable of "parsing data into any of the types Pydantic can handle as fields of a `BaseModel`" ([Pydantic, *TypeAdapter*](https://pydantic.dev/docs/validation/latest/concepts/type_adapter/)).

Look at what is being passed. `list[User]` is not a class. Neither is `int | None`. Neither is `Literal["draft", "published"]`. They are *type expressions*, handed around as ordinary runtime values.

The pre-3.15 vocabulary had nothing for this. `type[T]` means "a class object producing `T`", which excludes every example above. So the honest annotation was `Any`, and `Any` erases the return type — the single most useful thing the function knows.

PEP 747 closes the gap. The abstract defines it plainly: "`TypeForm[T]` describes the set of all type form objects that represent the type `T` or types that are assignable to `T`." The PEP's motivation names the libraries this exists for, listing `pydantic.TypeAdapter(T).validate_python` alongside `typeguard.check_type`, `beartype.is_bearable`, `trycast.isassignable`, and `cattrs.BaseConverter.structure`.

The payoff is that the type flows through:

```python
from typing import TypeForm

def validate[T](tp: TypeForm[T], value: object) -> T: ...

user = validate(User, payload)          # user: User
maybe = validate(int | None, raw)       # maybe: int | None
items = validate(list[User], body)      # items: list[User]
```

Every one of those return types is now known statically. Previously the third line gave you `Any` and every downstream line inherited it — the quiet way a fully annotated codebase stops being checked.

## PEP 728: saying what an API rejects

PEP 728's abstract is one sentence: "This PEP adds two class parameters, `closed` and `extra_items` to type the extra items on a TypedDict."

```python
from typing import TypedDict

class MovieClosed(TypedDict, closed=True):
    name: str

MovieClosed(name="No Country for Old Men")              # OK
MovieClosed(name="No Country for Old Men", year=2007)   # Not OK

class ExtraMovie(TypedDict, extra_items=int):
    name: str

ExtraMovie(name="No Country for Old Men", year=2007)    # OK
```

The reason this matters to anyone shipping an HTTP API is that "does this payload permit unknown keys?" was already a decision you were making — just only at runtime. Pydantic's `extra` config setting "can take three values: `'ignore'`: Providing extra data is ignored (the default). `'forbid'`: Providing extra data is not permitted. `'allow'`: Providing extra data is allowed and stored in the `__pydantic_extra__` dictionary attribute" ([Pydantic, *Models*](https://pydantic.dev/docs/validation/latest/concepts/models/)).

`closed=True` is the static expression of `extra='forbid'`. `extra_items=int` is the static expression of `extra='allow'` with a known value type. For the first time the strictness of your payload contract is a fact a type checker can read, rather than a runtime configuration it cannot see.

PEP 728's own motivation is more foundational than the API-schema use case — it is about letting checkers infer precise return types for `.items()` and `.values()`, narrow with `in` checks, and support additional keyword arguments with `Unpack`. But the practical effect for schema-shaped code is that the two halves of your contract can finally agree in writing.

## PEP 800: the one you will never type

`@typing.disjoint_base` is the PEP you are least likely to write yourself, and the [Python 3.15 release notes](https://docs.python.org/3.15/whatsnew/3.15.html) say so: it is "primarily intended to allow type checkers to faithfully reflect the runtime semantics of types defined as builtins or in compiled extensions."

The problem it fixes is narrowing that everyone agreed on but nobody had grounds for:

```python
def f(x: int):
    if isinstance(x, str):
        print("It's both!")   # unreachable — but why, formally?
```

CPython does not permit a class to inherit from both `int` and `str`, so that branch is dead. PEP 800 states the gap directly: "The information necessary to determine that these base classes are incompatible is not currently available in the type system." Checkers were reaching the right answer by special-casing builtins rather than by reading the type system.

The decorator makes the constraint declarable — including for third-party compiled extensions, which no checker can special-case. Its rule: "Two classes that have distinct, unrelated disjoint bases cannot have a common child class."

For dependency-injection and validator code, which is dense with `isinstance` guards over heterogeneous inputs, this is the difference between reachability analysis that happens to work on builtins and reachability analysis that works on your own types too.

## The gap this opens

Here is the uncomfortable part. A frozen feature set in CPython is the *start* of the work for everyone downstream, not the end of it.

`TypeForm`, `closed`, `extra_items`, and `disjoint_base` are runtime-importable in 3.15 whether or not the type checker in your editor understands what they mean. When it doesn't, you get the worst version of static typing: annotations that look precise, pass import, and are being interpreted by nothing. Meanwhile the runtime frameworks *are* interpreting them, so your editor and your production server now hold different beliefs about the same line of code.

That gap is part of what the [official `python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) exists to measure. Our integrity review has also shown that the suite alone is insufficient when an implementation has been developed against the exact fixtures; robustness and mutation testing are required as well.

## Where Basilisk stands

Basilisk is an open-source Python type checker and language server that adds code intelligence, formatting, type-aware refactoring, testing, debugging, and CPU and memory profiling to VS Code, Cursor, and Windsurf, with the same Rust language-server core behind Zed and Neovim.

All three Python 3.15 typing PEPs have conformance test files in the official suite. We previously published pass counts for those files and used them to assert support. **Those figures and that support claim are withdrawn.** Test-specific implementation logic elsewhere in the checker demonstrated that a pass against an exact fixture was not enough to establish a general implementation.

Basilisk's support status for `TypeForm`, `closed` / `extra_items`, and `disjoint_base` is therefore being revalidated as part of the integrity audit, and any rule that turns out to match source text is deleted rather than repaired. Until those rules pass semantics-preserving mutations and broader cases that were not present in the suite, do not rely on the old table. See the [conformance correction](/docs/conformance/) for the publication bar we will apply to the replacement result.

## What to do before October

Python 3.15's final release is scheduled for 2026-10-01. Between now and then:

- **Find your `Any`-typed type parameters.** Anywhere you pass a type expression as a value — a validator factory, a deserialiser, a settings loader — is a candidate for `TypeForm[T]`, and probably a place where inference is silently dying today.
- **Write down whether your payloads are closed.** If a model is configured to forbid extra keys at runtime, `closed=True` states the same thing where a checker can enforce it.
- **Check what your checker does with the new forms.** Import them, use them, and confirm you get a real diagnostic on deliberately wrong code. Silence is not a pass.
- **Adopt on the library's schedule, not CPython's.** These PEPs are final in CPython; when and how FastAPI, Pydantic, and your other annotation-driven dependencies expose them is each project's own decision. Follow their release notes.

The larger shift is worth naming. Python spent a decade treating annotations as optional metadata, and an ecosystem grew up that treats them as source code. Three PEPs in one release, all of them making annotations more expressive at runtime, is that ecosystem winning the argument.

Which raises the standard for the tools that read those annotations. If your type hints are going to run, something had better be checking them.

[Install Basilisk for VS Code](/docs/install-vscode/) · [Read the conformance correction](/docs/conformance/) · [Read the benchmark review notice](/docs/benchmarks/)
