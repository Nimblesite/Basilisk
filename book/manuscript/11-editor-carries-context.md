# Chapter 11 — Let the editor carry context

*Part III — Make it your workflow*

> **Reader promise:** Use language features as connected views over one
> workspace analysis, with a preview mindset for every change.

## Ask without leaving the code

Connect completion, signature help, hover, and inlay hints to the same source
and resolved type information used for diagnostics.

## Move through relationships

Demonstrate definition, references, symbols, call hierarchy, and type hierarchy
only where the documented release advertises and tests the capability.

## Preview workspace edits

Show rename and bounded refactors as proposed multi-file edits. State their
preconditions and known boundaries; do not promise arbitrary semantic rewrites.

## Keep source hygiene in the loop

Demonstrate the embedded Ruff formatter and native import organization through
the editor. Do not invent a `basilisk format` CLI command.

## Signal Box checkpoint

Navigate to a storage implementation, find all protocol uses, preview a rename,
perform one bounded extract, format the result, and re-check the workspace.

## Authoritative sources

- [Language Server Protocol 3.18](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/)
- Use the live [Basilisk refactoring guide](https://www.basilisk-python.dev/docs/refactoring/)
  for reader navigation, with feature claims verified in the release source and
  integration tests.
