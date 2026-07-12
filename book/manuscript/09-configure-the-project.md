# Chapter 9 — Configure the project, not a mood

*Part III — Make it your workflow*

> **Reader promise:** Express the project's actual target, file scope, and rule
> policy without reaching for an undefined “strictness mode.”

## Start from the project root

Define the working root and canonical `pyproject.toml` table. Run tutorial checks
from that root so configuration discovery is deterministic for the documented
release.

## Target the Python you run

Set the Python version deliberately. Keep Python 3.12 as this edition's
canonical path and label later-version examples rather than letting their
semantics leak into the main narrative.

## Choose the analyzed files

Cover include roots, stub paths, and custom typeshed only after their governing
specification, release implementation, and executable tests agree.

## Select rules individually

Separate default Python-spec rules from opt-in Basilisk policy rules. Enable
required-annotation rules explicitly for Signal Box, then show rule-level
severity rather than inventing basic/standard/strict modes.

## Signal Box checkpoint

Write one explicit policy for source, tests, generated fixtures, and simulated
vendor code. Predict the effective severity in one file from each path.

## Authoritative sources

- [pyproject.toml specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
- [Type checker directives](https://typing.python.org/en/latest/spec/directives.html)
- Use the live [Basilisk configuration guide](https://www.basilisk-python.dev/docs/configuration/)
  and [rule reference](https://www.basilisk-python.dev/docs/rules/), checked
  against the release used by this edition.
