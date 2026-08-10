# basilisk-test-utils

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

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

Consumed by the test suites of crates that ship in nothing.
