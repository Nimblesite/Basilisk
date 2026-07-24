# Chapter 12 — Run, investigate, and ship

*Part III — Make it your workflow*

> **Reader promise:** Combine static, test, debug, profiling, and CI evidence
> without asking one tool to answer another tool's question.

## Type checking is not execution

Open with four questions: is a value compatible, did an example pass, what state
exists at this line, and where did time or memory go? Route each to the right
evidence source.

## Discover and run tests

Use the implemented editor test workflow and document known release boundaries.
Keep pytest or unittest behavior tied to their own official documentation when
the chapter discusses the test framework rather than Basilisk's integration.

## Pause and inspect runtime state

Use the primary VS Code path to start a real debug session, stop at a Signal Box
breakpoint, and inspect relevant variables and the call stack. State clearly
where a Python interpreter and debug adapter are involved.

## Measure before optimizing

Capture the supported CPU profiling path and interpret one flame graph and one
source heat view. Describe the measurement and platform instead of presenting a
heuristic as ground truth.

## Repeat the same check in CI

Pin the documented Basilisk release. Show verified text and JSON output, actual
process status, the optional check cache, and the project-root invocation. Do
not include unimplemented `stats`, watch, SARIF, or JUnit commands.

## Signal Box checkpoint

Check, test, debug, profile, and CI the complete project. Record which question
each tool answered and one limitation that remains outside its evidence.

## Authoritative sources

- [Debug Adapter Protocol specification](https://microsoft.github.io/debug-adapter-protocol/specification)
- Use the live [Basilisk debugging](https://www.basilisk-python.dev/docs/debugging/),
  [profiling](https://www.basilisk-python.dev/docs/profiler/), and
  [release](https://www.basilisk-python.dev/docs/releases/) guides, checked
  against the edition's release.
