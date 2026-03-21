# basilisk-test-utils

Shared test helpers for Basilisk integration and E2E tests.

## Role in Basilisk

This crate provides **reusable test infrastructure** for the entire workspace. It contains helper functions for setting up test fixtures, running the analysis pipeline, and asserting on diagnostics — ensuring consistent, DRY test code across all crates.

## Key concepts

- **E2E test helpers** — functions to parse, resolve, and check Python snippets in a single call.
- **Diagnostic assertions** — helpers to assert that specific diagnostic codes are emitted at expected locations.
- **No unit tests** — following the project's testing philosophy, all tests are coarse E2E tests that exercise the full pipeline.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde_json` | Test output serialization |

## Status

Complete — consumed by test suites across the workspace.
