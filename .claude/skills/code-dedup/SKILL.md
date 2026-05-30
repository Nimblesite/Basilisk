---
name: code-dedup
description: Uses deslop MCP to find the worst duplicate-code offenders in the repo and merge them. Loops until there are no major duplication issues left. Use when the user says "deduplicate", "find duplicates", "remove dead code", "DRY up", or "code dedup".
---
<!-- agent-pmo:74cf183 -->

# Code Dedup

Drive deduplication through the **deslop MCP**. Start with `top-offenders`, fix the worst cluster, then loop on `top-offenders` again. Keep going until there are no major duplication issues left.

## Required tools

You MUST use these tools — do not substitute grep/find/manual searching:

- `mcp__deslop__top-offenders` — **start here every iteration.** The worst duplicate clusters in the repo.
- `mcp__deslop__cluster-by-id` — full member list for a specific cluster
- `mcp__deslop__find-similar` — confirm no near-duplicate was reintroduced after a merge
- `mcp__deslop__report-for-file` / `mcp__deslop__report-for-range` — verify a touched file/range
- `mcp__deslop__report-query` / `mcp__deslop__report-get` — drill into the current report

If a tool's schema isn't loaded, call `ToolSearch` with `select:mcp__deslop__<name>` first.

Do NOT call `mcp__deslop__rescan` at the start. Just call `top-offenders` directly.

## The loop

```
1. mcp__deslop__top-offenders          -> list of worst clusters
2. If list is empty / only minor      -> EXIT loop, report done
3. Pick the #1 cluster
4. mcp__deslop__cluster-by-id          -> all members
5. Merge the duplicates (extract shared code, update call sites, delete duplicates)
6. mcp__deslop__find-similar on the new shared code
   -> confirm it didn't re-introduce a near-duplicate elsewhere
7. Go back to step 1
```

"No major duplication issues" means `top-offenders` returns nothing worth merging under the rules below. One pass is not enough — keep looping.

## Per-iteration detail

### 1. Pull the worst offenders

Call `mcp__deslop__top-offenders` **first, every iteration**. Work the list top-down — biggest offender first.

### 2. Inspect the cluster

Call `mcp__deslop__cluster-by-id` for the top entry. Read every member. Confirm they are genuinely duplicated logic (not coincidental similarity).

### 3. Merge

- Extract shared logic into a single function/module (prefer moving to a shared crate per CLAUDE.md)
- Update all call sites
- Delete the now-unused duplicates
- Preserve public API surface — do not change exported signatures

### 4. Verify with deslop

- `mcp__deslop__find-similar` against the new shared code — confirm no near-duplicate was reintroduced
- `mcp__deslop__report-for-file` on the touched files — confirm the cluster is gone

### 5. Loop

Go back to step 1. Keep going until `top-offenders` has no major entries left.

## Filing deslop bugs / false positives

If deslop reports something that is clearly wrong — a cluster of code that is not actually duplicated, members that don't belong together, a crash, malformed output, missing files, stale results — **you MUST file a detailed issue against the deslop repo via `gh`**. Do not silently work around it.

Use:

```
gh issue create --repo <deslop-repo> --title "<concise summary>" --body "<details>"
```

The issue body MUST include:

- The exact deslop tool called and its full arguments
- The full response (or relevant excerpt) showing the false positive / bug
- File paths and line ranges of the cluster members
- Why this is a false positive (e.g. "members X and Y share only the function signature, bodies are unrelated") or a precise description of the bug
- Repo commit SHA so the maintainer can reproduce
- What you expected deslop to return instead

If you don't know the deslop repo URL, ask the user once and then proceed.

After filing, skip that cluster and continue the loop with the next offender.

## Rules

- **deslop drives the work.** No grep-based duplicate hunting. The MCP is the source of truth.
- **Start with `top-offenders`.** Never start the loop with `rescan` or anything else.
- **Loop until clean.** One pass is not enough. Stop only when `top-offenders` has no major entries left.
- **File issues on false positives / bugs.** Every deslop mistake gets a detailed `gh issue create` against the deslop repo.
- **When in doubt, leave it.** If a cluster's members look similar but aren't equivalent, file a false-positive issue and skip.
- **Preserve public API surface.** Do not change exported function signatures or module exports.
- **Three similar lines is fine.** Only merge when shared logic is substantial (>10 lines) or 3+ copies.
- **Move, don't copy.** Per CLAUDE.md, copying files is illegal — move them.
- **No `allow(clippy=...)`, no `unwrap`, no regex** — the merged code must meet the project's Rust quality bar.
