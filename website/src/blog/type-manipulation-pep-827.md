---
layout: layouts/blog.njk
title: "PEP 827 Explained: Conditional and Mapped Types for Python"
description: "PEP 827 could bring conditional types, mapped types, and type-level programming to Python. We explain the draft proposal and what it means for Basilisk."
date: 2026-07-16
dateModified: 2026-07-16
author: The Basilisk Project
image: /assets/images/blog/pep-827-type-manipulation.png
imageAlt: "Python type shapes passing through a conditional evaluator and expanding into a mapped type lattice"
imageWidth: 1200
imageHeight: 675
tags:
  - Python typing
category: deep-dives
excerpt: "PEP 827 is Python's boldest attempt yet at TypeScript-style type manipulation. It is powerful, controversial, and a serious implementation challenge for every Python type checker."
keywords: PEP 827, Python typing, conditional types Python, mapped types Python, type manipulation, Python type system, TypeScript conditional types, Python type checker, static typing, Basilisk
faq:
  - q: "What is PEP 827?"
    a: "PEP 827 is a draft proposal for type manipulation in Python. It introduces conditional types, mapped types, type-level operators, member introspection, and tools for constructing new protocols and TypedDicts."
  - q: "Does Python support conditional or mapped types today?"
    a: "Not in the form proposed by PEP 827. The PEP is still a draft, and its new operators and type-expression rules are not part of the released Python typing specification."
  - q: "Is PEP 827 accepted?"
    a: "No. As of July 16, 2026, PEP 827 is a draft under active discussion. Its details may change, it may be divided into smaller proposals, or it may not be accepted."
  - q: "Does Basilisk support PEP 827?"
    a: "No. PEP 827 is not an accepted part of the Python typing specification. Basilisk is following the proposal and will make implementation decisions as its semantics and conformance requirements stabilize."
---

[PEP 827](https://peps.python.org/pep-0827/) is the most ambitious Python typing proposal in years. It would add conditional types, mapped types, type-level introspection, recursive type aliases, custom type errors, and roughly twenty new operators to `typing`.

That is a lot. It is also the clearest attempt yet to close the gap between Python's dynamic programming model and the much richer type manipulation available in TypeScript.

We think the direction is right. Library authors should be able to describe what their APIs do without writing a separate plugin for every type checker. But the current proposal asks type checkers to grow something close to a type-level interpreter, and the cost will show up in implementation complexity, performance, error messages, and maintenance.

PEP 827 is still a draft. Basilisk does not implement it today, and nobody should plan production code around it yet. Here is what the proposal actually does, why people care, and what would have to happen inside a checker like Basilisk.

Michael J. Sullivan, Daniel W. Park, and Yury Selivanov created the Standards Track proposal in February 2026, with Python 3.16 listed as its target. The [PEP source is public in `python/peps`](https://github.com/python/peps/blob/main/peps/pep-0827.rst), and the authors introduced the work in [Vercel's announcement](https://vercel.com/blog/advancing-python-typing). A target version is not an acceptance decision: the proposal remains under active revision.

## What problem is PEP 827 trying to solve?

Python libraries routinely generate or transform classes at runtime. ORMs derive model fields, validation frameworks inspect annotations, decorators rewrite call signatures, and dataclass-like tools synthesize methods. Python can express all of that at runtime, but its static type system often cannot describe the result.

The usual escape hatch is a checker-specific plugin. That works for a library that is willing to target mypy, but it does not create portable typing behavior across mypy, Pyright, Pyrefly, ty, and Basilisk. PEP 827 argues that the shared extension point should be the standard `typing` vocabulary, not a collection of incompatible plugin APIs.

There is evidence that users want this expressiveness. The [2025 Typed Python Survey](https://engineering.fb.com/2025/12/22/developer-tools/python-typing-survey-2025-code-quality-flexibility-typing-adoption/) lists TypeScript-inspired features among the most requested additions, including intersection types, mapped and conditional types, and utilities such as `Pick`, `Omit`, `keyof`, and `typeof`.

This is not the first time Python has standardized its way out of a plugin problem. [`@dataclass_transform`](https://peps.python.org/pep-0681/) gave checkers a common way to understand libraries that behave like dataclasses. PEP 827 takes the same instinct much further: instead of standardizing one transformation at a time, give library authors a vocabulary for describing transformations themselves.

## Conditional types in Python

The headline feature is a type expression that chooses one result or another after evaluating a type-level condition:

```python
type ConvertField[T] = (
    AdjustLink[PropsOnly[PointerArg[T]], T]
    if typing.IsAssignable[T, Link]
    else PointerArg[T]
)
```

It looks like an ordinary Python conditional expression, but the condition is a restricted *type boolean*. Operators such as `IsAssignable[T, Link]` resolve to `Literal[True]` or `Literal[False]` during type checking.

This is the Python counterpart to a TypeScript conditional type such as `T extends S ? X : Y`. It reuses Python syntax, but it changes which expressions are valid in a type annotation. The [PEP's grammar](https://peps.python.org/pep-0827/#grammar-specification-of-the-extensions-to-the-type-language) also permits type booleans to be combined with `and`, `or`, `not`, `all`, and `any`.

## Mapped types and type introspection

Mapped types are built from unpacked comprehensions over the members of another type. The PEP demonstrates familiar TypeScript utilities in Python:

```python
# Pick[T, Keys]
type Pick[T, Keys] = typing.NewProtocol[
    *[
        member
        for member in typing.Iter[typing.Members[T]]
        if typing.IsAssignable[member.name, Keys]
    ]
]

# KeyOf[T]
type KeyOf[T] = Union[
    *[member.name for member in typing.Iter[typing.Members[T]]]
]

# Partial[T] — make every property optional
type Partial[T] = typing.NewProtocol[
    *[
        typing.Member[
            member.name,
            member.type | None,
            member.quals,
        ]
        for member in typing.Iter[typing.Attrs[T]]
    ]
]
```

`Members[T]` exposes a type's members. The comprehension filters or transforms them. `NewProtocol` and `NewTypedDict` construct a new type from the result.

The proposal goes well beyond those two examples. It includes operators for inspecting type arguments, attributes, callable parameters, tuple lengths and slices, unions, protocols, TypedDicts, and overloaded functions. `RaiseError` would let a type-level program produce a custom diagnostic instead of collapsing into an opaque `Never` error.

It also permits recursive aliases. The PEP's `Zip` example walks two tuple types from the end, recursively builds a tuple of pairs, and reports a custom error when their lengths differ:

```python
type Zip[T, S] = (
    tuple[()]
    if typing.Bool[Empty[T]] and typing.Bool[Empty[S]]
    else typing.RaiseError[Literal["Zip length mismatch"], T, S]
    if typing.Bool[Empty[T]] or typing.Bool[Empty[S]]
    else tuple[
        *Zip[DropLast[T], DropLast[S]],
        tuple[Last[T], Last[S]],
    ]
)
```

That example is useful because it shows the real scale of the proposal. This is not just `Pick` and `keyof` with Python spelling. It is a language for writing recursive programs over types.

There is no Python grammar change here. The expressions already parse as Python; PEP 827 broadens the subset of expressions that type checkers accept as types.

### Where Python deliberately differs from TypeScript

PEP 827 borrows the broad idea from TypeScript without copying its type system. TypeScript checks constraints such as `P[K]` at the alias definition site, where `K` must be a key of `P`. PEP 827 allows more expressions to remain unresolved until the alias is expanded, so an alias can fail for a particular set of arguments.

The authors also rejected TypeScript's `infer`-based pattern matching because it fits poorly into Python's existing syntax. In the other direction, Python's proposed [`RaiseError`](https://peps.python.org/pep-0827/#raiseerror) has no direct TypeScript equivalent: it lets a library author attach a useful compile-time message to an invalid expansion rather than forcing the user to reverse-engineer why a type became `Never`.

### Runtime annotations still matter

Python annotations are not only checker input. Pydantic, FastAPI, and other frameworks inspect them at runtime, which is a constraint TypeScript does not share. The proposal therefore includes a [`typing.special_form_evaluator`](https://peps.python.org/pep-0827/#runtime-evaluation) hook that would let a runtime library evaluate special forms such as type booleans and `Iter`.

The standard library would provide the hook, not a complete evaluator. The authors have published a [runtime prototype in `python-typemap`](https://github.com/vercel/python-typemap), while Michael Sullivan has an [experimental mypy implementation](https://github.com/msullivan/mypy/tree/typemap). These are proofs of direction, not a compatibility promise.

## Why not just use normal Python functions?

It sounds attractive: write a function that accepts a type and returns another type, then let the checker run it. The problem is deciding which Python code is safe, deterministic, and practical for every checker to execute.

A checker written in Rust or TypeScript should not need to embed CPython to understand a library's annotations. A restricted "type function" language would still be a new language, even if it happened to use `def` syntax. PEP 827 instead defines a closed set of operations that static checkers and runtime annotation frameworks can implement independently.

That trade-off is reasonable. It is not cheap.

## The real cost is in the checker

PEP 827 is often described as a syntax proposal. From a type checker's side, it is an evaluation engine.

A checker would need to:

- evaluate type-level boolean conditions;
- introspect and construct members, parameters, protocols, and TypedDicts;
- distribute many operations across every member of a union;
- evaluate recursive aliases without hanging;
- represent partially evaluated, or "stuck," expressions containing unresolved type variables;
- cache results aggressively enough to keep editor feedback fast; and
- explain failures in terms a developer can act on.

The union case alone can grow quickly. If an operator accepts several union-valued arguments, evaluation may require their cross-product before the results can be normalized back into a union. Add recursion and member comprehensions, and a small annotation can trigger a surprising amount of work.

The PEP calls this behavior *lifting over unions*. An operator is evaluated for each combination of union members, then the results are joined back together. That is clean on paper and potentially explosive in a large program.

Unresolved type variables create another problem. An expression such as `GetArg[T, Base, 0]` cannot always be reduced while `T` is still unknown. The checker has to preserve that partially evaluated, or *stuck*, expression and compare it consistently later. The mypy prototype currently treats stuck operators invariantly, but that behavior is part of an implementation under development, not a settled cross-checker rule.

The draft does not yet specify a universal evaluation budget or recursion limit. That matters. The [termination discussion](https://discuss.python.org/t/pep-827-type-manipulation/106353) notes that recursive type aliases can already consume large amounts of time and memory, but PEP 827 gives programmers many more ways to create expensive expansions. Every production checker will need a deterministic answer for pathological or simply accidental type-level programs.

For Basilisk, the natural design would be a bounded, memoized evaluator integrated with our incremental query system. Content-addressed inputs give us a good foundation for caching, but architecture does not make the semantic questions disappear. We still need stable rules for normalization, recursion, diagnostics, and partially evaluated types before implementation would be responsible.

## The disagreement is real

The [discussion on Python.org](https://discuss.python.org/t/pep-827-type-manipulation/106353) is unusually divided.

Framework authors can see immediate uses. [Sebastián Ramírez pointed to FastAPI, SQLModel, Typer, machine-learning libraries, and distributed task systems](https://discuss.python.org/t/pep-827-type-manipulation/106353/2) as projects that could benefit. That makes sense: these are exactly the kinds of libraries whose runtime APIs outpace what today's static types can express.

The objections are substantial too. [Victorien described the scale as ambitious](https://discuss.python.org/t/pep-827-type-manipulation/106353/6), pointing out that the proposal introduces roughly twenty special forms. [Steve Dower's response](https://discuss.python.org/t/pep-827-type-manipulation/106353/12) was blunter: he did not want to read the ORM example or implement the static evaluator required to calculate it. Other reviewers raised the prospect of TypeScript- or C++-style error messages and the performance cost of expanding complex type programs.

[Ivan Levkivskyi, a mypy core developer, came out strongly against the proposal](https://discuss.python.org/t/pep-827-type-manipulation/106353/53) and made clear that mypy had not endorsed it. After reviewing common mypy and typing feature requests, he argued that mapped types would help only a small number and that the proposal did little for numeric and scientific typing. Michael H also raised deeper semantic objections around variance, invalid operations becoming `Never`, and generic callables.

The authors' strongest answer is portability. [Yury Selivanov argued](https://discuss.python.org/t/pep-827-type-manipulation/106353/13) that a common plugin API is unrealistic when mypy is written in Python, ty in Rust, and Pyright in TypeScript. Every checker already has to understand `typing`; extending that shared language is more plausible than persuading every implementation to expose the same internal plugin surface.

Both sides are responding to real pain. Library authors are tired of the ceiling imposed by today's type system. Checker maintainers and users are tired of type errors that require a compiler engineer to decode. PEP 827 expands the power of the system, and power always spends some of its complexity budget downstream.

## Our position

Python needs a portable way to describe type transformations. Checker-specific plugins cannot be the long-term answer when the major checkers are written in different languages and serve different editors and build systems.

PEP 827 also needs tightening before it is ready. The core ideas are compelling, but the proposal combines several separable features: conditional and mapped types, callable manipulation, keyword-argument inference, overload construction, class updates, custom errors, and runtime evaluation hooks. Smaller proposals could deliver useful parts sooner and give the typing ecosystem room to learn from actual implementations.

There is precedent for splitting the work. PEP 827 builds on [`ParamSpec`](https://peps.python.org/pep-0612/), [variadic generics and `Unpack`](https://peps.python.org/pep-0646/), [TypedDict keyword arguments](https://peps.python.org/pep-0692/), and [deferred annotation evaluation](https://peps.python.org/pep-0649/). It also intersects with the draft [inline TypedDict proposal](https://peps.python.org/pep-0764/) and ongoing work on intersection types. The `**kwargs` inference and extended-callable pieces could move independently without waiting for the full recursive evaluator.

Basilisk will follow the draft, its reference implementations, and any conformance tests that emerge. The sensible order is to understand the inert introspection and construction operators first, then conditional evaluation and union normalization, and leave the most difficult recursive and class-mutating features until their semantics settle. Any future support claim must be grounded in the typing specification, the official [`python/typing` conformance suite](https://github.com/python/typing/tree/main/conformance), and robustness testing that shows the implementation is not fitted to the exact fixtures.

We are not going to claim support for a moving draft. If PEP 827, or a smaller proposal derived from it, enters the Python typing specification, Basilisk will implement it against the same public conformance process we use for the rest of the type system.

## What happens next?

As of July 16, 2026, [PEP 827 remains a draft](https://peps.python.org/pep-0827/). Its stated target is Python 3.16, but a target is not an acceptance decision. Under Python's [typing governance process](https://peps.python.org/pep-0729/), the Typing Council decides whether typing PEPs are accepted. No acceptance decision is recorded for PEP 827, and the proposal can still change substantially, split into smaller pieces, or be rejected.

That uncertainty should not obscure the signal. Python library authors want types that can keep up with Python code. Whether PEP 827 lands intact or becomes the starting point for several narrower proposals, conditional types and mapped types are now part of the serious conversation about Python's type system.

That conversation is worth having. The hard part is making the result powerful without making ordinary Python development miserable.
