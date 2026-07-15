# Chapter 3 — Read the evidence

*Part I — See the system*

> **Reader promise:** Read a diagnostic as evidence about one type
> relationship, decide what needs human judgment, and verify the result.

## Read from the outside in

Introduce severity, rule identifier, message, source location, span, help,
note, and documentation destination using one real captured diagnostic. Explain
which elements are stable identifiers and which wording may change by release.

## Find the relationship beneath the symptom

Teach the reader to name the source type, destination type, and operation before
editing. The first diagnosis exercise stops before the fix so prediction remains
part of the lesson.

## Use hover as another view

Show the same relationship in editor hover and terminal output. Hover provides
context; it does not override the declared source or the diagnostic evidence.

## Fixes are proposals

Separate safe mechanical edits from choices about domain meaning. Preview an
editor action, inspect its diff, and re-check. Do not imply that generated
placeholder annotations are production design decisions.

## Signal Box checkpoint

Diagnose an argument mismatch, a return mismatch, and an unresolved import.
Write down the relationship first; then apply the smallest honest change.

## Authoritative sources

- [Type checker directives](https://typing.python.org/en/latest/spec/directives.html)
- Use the live [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/)
  and the per-diagnostic link printed by the documented release.
