---
name: test
description: Runs the full test suite and reports results including coverage. Use when the user asks to run tests, check test coverage, or verify that code changes pass all tests.
---

# Test

Run the full test suite and report results.

## Steps

1. Run `make test`
2. Run `make coverage-check`
3. Report: total tests, passed, failed, skipped, coverage %
4. If any test fails, show the full failure output

## Rules

- Never delete or modify tests to make them pass
- Never skip tests
- Fix the code, not the test

## Success criteria

- All tests pass
- Coverage meets threshold
- No tests skipped
