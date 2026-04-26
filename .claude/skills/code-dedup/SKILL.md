---
name: code-dedup
description: Searches for duplicate code, duplicate tests, and dead code, then safely merges or removes them. Use when the user says "deduplicate", "find duplicates", "remove dead code", "DRY up", or "code dedup". Requires test coverage — refuses to touch untested code.
---
<!-- agent-pmo:2efd847 -->

# Code Dedup

Carefully search for duplicate code, duplicate tests, and dead code across the repo. Merge duplicates and delete dead code — but only when test coverage proves the change is safe.

## Prerequisites — hard gate

Before touching ANY code, verify these conditions. If any fail, stop and report why.

1. Run `make test` — all tests must pass (includes coverage thresholds). If tests fail, stop. Do not dedup a broken codebase.
2. Rust and TypeScript are statically typed — proceed.

## Steps

Copy this checklist and track progress:

```
Dedup Progress:
- [ ] Step 1: Prerequisites passed (tests green, coverage met, typed)
- [ ] Step 2: Dead code scan complete
- [ ] Step 3: Duplicate code scan complete
- [ ] Step 4: Duplicate test scan complete
- [ ] Step 5: Changes applied
- [ ] Step 6: Verification passed (tests green, coverage stable)
```

### Step 1 — Inventory test coverage

1. Run `make test` to confirm green baseline (includes coverage thresholds)
2. Note the current coverage percentage — this is the floor. It must not drop.
3. Identify which files/modules have coverage and which do not. Only files WITH coverage are candidates for dedup.

### Step 2 — Scan for dead code

1. Look for unused exports, unused functions, unused classes, unused variables
2. Use language-appropriate tools:
   - Rust: the compiler already warns on dead code — check `make lint` output
   - TypeScript: check for unexported functions with zero references
   - Python: look for functions/classes with zero imports across the codebase
3. For each candidate: **grep the entire codebase** for references (including tests, scripts, configs). Only mark as dead if truly zero references.
4. List all dead code found with file paths and line numbers. Do NOT delete yet.

### Step 3 — Scan for duplicate code

1. Look for functions/methods with identical or near-identical logic
2. Look for copy-pasted blocks (same structure, maybe different variable names)
3. Look for multiple implementations of the same algorithm or pattern
4. Check across module boundaries — duplicates often hide in different crates
5. For each duplicate pair: note both locations, what they do, and how they differ
6. List all duplicates found. Do NOT merge yet.

### Step 4 — Scan for duplicate tests

1. Look for test functions with identical assertions against the same code paths
2. Look for test fixtures/helpers that are duplicated across test files
3. List all duplicate tests found. Do NOT delete yet.

### Step 5 — Apply changes (one at a time)

For each change: **change -> test -> verify coverage -> continue or revert**.

#### 5a. Remove dead code
- Delete dead code identified in Step 2
- After each deletion: run `make test`
- If tests fail or coverage drops: **revert immediately**

#### 5b. Merge duplicate code
- Extract shared logic into a single function/module
- Update all call sites
- After each merge: run `make test`
- If tests fail: **revert immediately**

#### 5c. Remove duplicate tests
- Delete the redundant test (keep the more thorough one)
- After each deletion: run `make test`
- If coverage drops: **revert immediately**

### Step 6 — Final verification

1. Run `make test` — all tests must still pass
2. Run `make test` — coverage must be >= the baseline from Step 1
3. Run `make lint` and `make fmt-check` — code must be clean
4. Report: what was removed, what was merged, final coverage vs baseline

## Rules

- **No test coverage = do not touch.** If a file has no tests covering it, leave it alone entirely.
- **Coverage must not drop.** The coverage floor from Step 1 is sacred.
- **One change at a time.** Make one dedup change, run tests, verify coverage. Never batch.
- **When in doubt, leave it.** If two code blocks look similar but you're not sure they're identical, leave both.
- **Preserve public API surface.** Do not change function signatures or module exports.
- **Three similar lines is fine.** Only dedup when shared logic is substantial (>10 lines) or 3+ copies.
