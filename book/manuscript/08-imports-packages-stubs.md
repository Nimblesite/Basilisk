# Chapter 8 — Imports, packages, and the world of stubs

*Part II — Think in types*

> **Reader promise:** Explain where imported type information came from and
> choose an honest response when a dependency has none.

## Runtime modules and static information

Separate what Python imports at runtime from the source or stub information a
type checker analyzes. Use the same simulated sensor package in both lanes.

## A stub is a public contract

Introduce `.pyi` as a description of a module's public interface. Keep the
example small enough that the reader can compare it directly with runtime
behavior.

## Typeshed and the standard library

Explain typeshed's role and which source the documented release actually used.
Checking is offline: by default Basilisk uses the complete `stdlib/` snapshot
compiled into the release and reports it as unpinned. Show `typeshed-commit` as
the way a project makes standard-library information reproducible — a pin
verifies, offline, that the tree in the local store hashes to that commit — and
state that it fails closed with `NO SOURCE` rather than downloading anything or
silently substituting another commit.

## Typed distributions and `py.typed`

Use the maintained distribution specification for package markers and stub
packages. Use PEP 561 for history only.

## Search order and provenance

Show the six implemented resolution steps in order — manual `stub-paths`, user
code, the selected standard-library source, stub packages, inline `py.typed`
packages, then vendored third-party stubs — and make clear that step 3 selects
exactly one source: a custom `typeshed-path`, an exact `typeshed-commit`, the
verified latest commit, or the bundled snapshot. Show hover provenance
distinguishing `(typeshed)` from `(custom typeshed)`. Verify every detail
against the release implementation and tests before final prose.

## Generate, then inspect

Demonstrate the shipped stub command using captured help. Treat generated output
as a draft contract that needs review and maintenance.

## Signal Box checkpoint

Add a local stub for the simulated vendor sensor package, correct one inaccurate
member, and verify the imported hover and project check.

## Authoritative sources

- [Distributing type information](https://typing.python.org/en/latest/spec/distributing.html)
- [Typeshed](https://github.com/python/typeshed)
- [PEP 561](https://peps.python.org/pep-0561/)
- [pyproject.toml specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
- Continue with the live [Basilisk configuration guide](https://www.basilisk-python.dev/docs/configuration/).

