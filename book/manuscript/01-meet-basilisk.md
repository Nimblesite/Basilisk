# Chapter 1 — Meet Basilisk

*Part I — See the system*

> **Reader promise:** Explain where Basilisk sits between Python source, project
> policy, an editor, the command line, and a running program.

![Python source and project configuration flow through Basilisk's parser, resolver, and checker to CLI diagnostics and editor language features.](../assets/diagrams/01-basilisk-system-map.png)

## One program, several questions

Open with the difference between asking Python to execute a program and asking
a static analyzer whether one value is compatible with a declared boundary.
Make the limit explicit: static analysis is evidence, not a proof of runtime
correctness.

## One engine, several surfaces

Walk the reader through Python source and active configuration, parsing and
resolution, rule selection, diagnostics and actions, the CLI/CI path, and the
language-server/editor path. Keep implementation detail subordinate to what the
reader can observe.

## Python rules and project policy

Establish the book's central configuration model: the unconfigured baseline
selects Python typing-spec rules. Additional Basilisk policy rules—such as
requiring annotations—are choices enabled individually. Do not call these two
sets “basic” and “strict” modes.

## Signal Box checkpoint

Open the completed sample without changing it. Identify the project root,
Python files, active configuration, terminal entry point, editor connection,
and runtime entry point.

## Practice

Ask readers to classify five questions as static-analysis, execution, test,
debugging, or profiling questions before the answers are revealed.

## Authoritative sources

- [The Python type system](https://typing.python.org/en/latest/spec/type-system.html)
- [Python 3.12 typing documentation](https://docs.python.org/3.12/library/typing.html)
- [Language Server Protocol 3.18](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/)
- Continue at the [Basilisk website](https://www.basilisk-python.dev/).

