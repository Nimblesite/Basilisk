---
name: build
description: Builds all artifacts in this repo including the Rust workspace and VS Code extension. Use when the user asks to build, compile, or package the project.
---

# Build

Build all project artifacts.

## Steps

1. Run `make build`
2. Report build outcome (success or failures with full error output)

## Rules

- Never suppress compiler warnings — all warnings are errors
- If the build fails, fix the code and re-run

## Success criteria

- `make build` exits with code 0
- Zero warnings output
