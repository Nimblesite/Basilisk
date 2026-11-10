# Chapter 10 — Adopt a codebase without hiding it

*Part III — Make it your workflow*

> **Reader promise:** Move existing code toward the chosen policy while keeping
> unfinished work visible and reviewable.

## Inventory before editing

Begin with a complete project-root check and group work by boundary and rule.
Do not introduce an invented coverage command or percentage.

## Apply bounded fixes first

Use captured `basilisk fix` help and a small diff to distinguish safe mechanical
edits from annotations or design choices that require review. Keep unsafe fixes
explicit.

## Adopt the honest remainder

Demonstrate the shipped `adopt`, `adopt --status`, and `unadopt` workflow. Show
how remaining diagnostics change severity rather than disappearing.

## Work from boundaries inward

Type external input, public functions, and module interfaces before local
implementation detail. Use Signal Box's vendor boundary to show why this order
reduces repeated guesses.

## Review generated annotations

Treat a placeholder such as `Any` or `None` as a visible prompt for a decision,
not proof that the tool inferred the domain correctly.

## Signal Box checkpoint

Migrate the deliberately untyped legacy module: baseline, bounded fix, human
review, adoption of the remainder, and one file returned to full severity.

## Authoritative sources

- [Type annotations](https://typing.python.org/en/latest/spec/annotations.html)
- Follow the live [Basilisk migration guide](https://www.basilisk-python.dev/docs/migration/)
  only for commands confirmed in the documented release.

