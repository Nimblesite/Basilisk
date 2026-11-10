# Test Failure Report

**Run date:** 2026-08-09

**Timeout policy:** every targeted hang/audit probe is killed after **10 seconds**

**Status:** the workspace test graph still does **not** complete

## Evidence snapshot

The unfiltered command remains:

```text
cargo test --workspace --all-targets --no-fail-fast
```

It compiled and entered the test graph, but
`workspace_scan_survives_deeply_chained_binary_expression` did not terminate.
A standalone rerun of that exact test hit the 10-second deadline.

A second run skipped the eight already-known 10-second timeouts. It completed
140 test harnesses, then stopped making progress inside `ws_core_tests`:

| Completed-harness evidence | Count |
|---|---:|
| Tests executed | 5,524 |
| Passed | 1,435 |
| Failed | 4,089 |
| Ignored | 8 |
| Filtered | 8 |
| Harnesses with failures | 93 |
| Harnesses without executed failures | 47 |

These are **partial totals**, not a workspace pass rate. The incomplete
`ws_core_tests` harness and all later harnesses are excluded.

The worktree changed while the long runner was executing. A final current-tree
probe was therefore run as:

```text
gtimeout 10s cargo test --workspace --all-targets --no-fail-fast
```

That probe reached the 10-second limit while recompiling changed crates, before
any tests ran. It does not supersede the completed-harness evidence above, and
it does not prove the newest tree compiles.

## Confirmed 10-second timeouts

The following eight tests each exceeded 10 seconds when run alone against the
captured test snapshot:

- `workspace_scan_survives_deeply_chained_binary_expression`
- `expand_wildcard_import_works_with_no_ruff_binary_on_path`
- `formatter_none_setting_disables_formatting_capabilities`
- `formatting_respects_tool_ruff_format_options`
- `formatting_works_with_no_ruff_binary_on_path`
- `organize_imports_works_with_no_ruff_binary_on_path`
- `range_formatting_works_with_no_ruff_binary_on_path`
- `split_multi_import_works_with_no_ruff_binary_on_path`

Six additional `ws_core_tests` cases panicked and then failed to shut down
within 10 seconds:

- `test_ws_file_watcher_triggers_reanalysis_in_whole_module`
- `test_ws_whole_module_startup_scan_publishes_diagnostics`
- `test_ws_type_checking_disabled_clears_module_tree_error_counts`
- `test_ws_type_checking_reenabled_republishes_diagnostics`
- `test_ws_configured_exclude_replaces_defaults_like_the_cli`
- `test_ws_unset_exclude_still_skips_default_excluded_trees`

Those six encountered the captured `typevar_defaults` panic first. That panic
was replaced after the run, so their current hang status requires a fresh exact
probe after the current tree can compile inside the deadline.

## Root-cause clusters

Counts below are panic **log occurrences**, not independent failed tests. One
test may emit several panic lines through worker threads or child processes.

| ID | Captured occurrences | Root site / semantic gap | Current worktree status |
|---|---:|---|---|
| P1 | 1,208 | `assignment_compatibility/default_spec.rs:85` — TypeVar defaults were selected from a rendered bare name | Rebuilt after the run from the default expression via `TypeNode::lower`; verification pending |
| P2 | 1,176 | `types_parsing.rs:51` — `InferredType::from_annotation` callers still hold raw annotation text | **Live**; callers must carry the annotation `Expr` into `AnnotationResolver` |
| P3 | 572 | `assignment_compatibility/alias_match.rs:421` — `alias_base` strips quotes and splits rendered generics | **Live**; alias identity must come from AST/bindings |
| P4 | 539 | `assignment_compatibility/sig_model.rs:161` — base identity was rendered-name keyed | Rebuilt after the run to lower parameter annotations through `BindingTable`; verification pending |
| P5 | 400 | `bidir/builtins.rs:49` (304) and `bidir/engine.rs:379` (96) — builtin/class-object recognition was spelling keyed | Rebuilt after the run around canonical forms and resolved leaves; verification pending |
| P6 | 12 | `types.rs:493` — two `InferredType::Named` values still cannot be compared by resolved class identity | **Live** mandatory panic |
| P7 | 27 | `subtyping.rs` (11), `stub_constructor.rs:113` (5), `generics_basic_3/helpers.rs:92` (3), and `exports.rs` (8) | **Live** deleted relation/MRO boundaries |
| P8 | 85 | tuple compatibility (38), missing attribute annotation (22), builtin methods (14), starred tuples (7), and guards (4) | Captured lower-volume deletion boundaries; verify individually after P2/P3 |
| **Total** | **4,019** | Mandatory implementation-panic occurrences in the completed-harness log | Not a failed-test count |

The captured 4,089 failures are therefore blast radius, not 4,089 distinct
typing defects. The three high-volume implementations changed after the log was
captured, so their old occurrence counts must not be presented as current
failure counts or subtracted to invent a new total.

## What changed since the previous report

- The former dominant `typeddict_meta.rs:77` panic (5,037 old log
  occurrences) is gone. That source file no longer exists, and the captured
  run contains no occurrence of that panic.
- The old 7,176-test “authoritative completed run” claim was deleted. The
  current workspace run is incomplete and only partial totals are defensible.
- The per-binary failure leaderboard was deleted because it measured shared
  panic blast radius, not independent rule failures.
- Current-source status is now kept separate from captured-run evidence so a
  post-run implementation cannot be mislabeled as a live panic.

## Audit gates

Both audit targets completed inside the 10-second deadline:

- `no_type_spelling_surgery_tests`: **1 passed / 1 failed** in 0.17 seconds.
  It reports **12 live sites** deriving type behaviour from spelling rather
  than resolved bindings.
- `no_assertion_debt_tests`: **0 passed / 1 failed** in 0.09 seconds. It
  reports **1,733 no-assertion patterns across 117 files**.

These guards are real failures. Do not make them green by weakening the scans,
restoring text parsing, deleting tests, or accepting silent `Unknown` results.

## Next verification

1. Remove the live raw-text boundaries at P2 and P3 by carrying AST nodes and
   resolved identities to their consumers.
2. Verify the post-run P1/P4/P5 replacements with focused tests.
3. Rerun each of the 14 timeout cases with the same 10-second hard limit.
4. Only then rerun the unfiltered workspace command and publish complete
   totals. Until it terminates, this report must stay explicitly partial.
