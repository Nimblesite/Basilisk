---
name: bugfix
description: Fix a bug using test-driven methodology. Enforces writing a failing test first, confirming it fails for the right reason, then fixing the bug without modifying the test. Use when fixing bugs, resolving issues, or when the user says "fix bug" or "bugfix".
disable-model-invocation: true
argument-hint: [description of the bug]
---

# Test-Driven Bug Fix Process

Fix the bug described in $ARGUMENTS using a strict test-first methodology. Follow each phase in order. Do NOT skip phases.

## Phase 1: Understand the Bug

1. Reproduce or clearly understand the bug from the description
2. Identify the relevant code and the root cause
3. Do NOT touch the production code yet

## Phase 2: Write a Failing Test

1. Write a test that **fails because of the bug**
2. The test must be a coarse/e2e test (not a unit test) per project conventions
3. The test must assert the **correct expected behavior** — the behavior we want after the fix
4. Run the test:

```bash
cargo test <test_name>
```

5. Confirm the test **fails**
6. Confirm it fails **because of the bug**, not for some other reason (wrong setup, typo, unrelated error)
7. If it fails for the wrong reason, fix the test and repeat from step 4
8. Once the test fails for the right reason, proceed to Phase 3

**IMPORTANT: After Phase 2 is complete, the test is LOCKED. You are NOT allowed to modify it.**

## Phase 3: Fix the Bug

1. Modify only production code to fix the bug
2. Do NOT change the test from Phase 2 — it is locked
3. Run the test:

```bash
cargo test <test_name>
```

4. If the test still fails, iterate on the production code fix only
5. Repeat until the test passes

## Phase 4: Verify

1. Run the full test suite to ensure no regressions:

```bash
cargo test
```

2. Run clippy:

```bash
cargo clippy --all-targets
```

3. Fix any issues found

## Rules

- NEVER modify the test after Phase 2 is complete
- NEVER delete or weaken test assertions
- NEVER skip writing the test first
- The test must fail for the RIGHT reason before you start fixing
- Only production code changes are allowed in Phase 3
