# Working Mutation Test Targets

This file tracks mutation slices that have been verified to complete and produce a mutation score. Append to the verified list only after running the Make target and confirming `mutation_testing/mutation_scores.csv` was updated.

## Verified From Existing Scores

| Crate | Make target | Evidence | Notes |
| --- | --- | --- | --- |
| basilisk-checker | `make mutation-run-checker-rule MUTATION_RULE=e0014` | `2026-03-16,checker/e0014,28,13,2,0,13,87%` | Historical score exists. Re-run before using as a CI gate. |
| basilisk-checker | `make mutation-run-checker-rule MUTATION_RULE=e0048` | `2026-03-16,checker/e0048,95,53,9,0,33,85%` | Historical score exists. Re-run before using as a CI gate. |
| basilisk-parser | `make mutation-run-crate-parser` | `2026-03-16,parser,2,0,0,0,2,0%` | Historical score exists, but all recorded mutants were unviable. |

## Historical Scores Without Current Make Targets

| Crate | Score row | Notes |
| --- | --- | --- |
| basilisk-checker | `2026-03-16,checker/e0097+e0150,95,25,37,0,33,40%` | Needs a dedicated verified target before it is added above. |
| basilisk-checker | `2026-03-16,checker/types+collection,21,0,21,0,0,0%` | Current checker script excludes broad shared type files as too slow or unviable. |

## Pending Verification

| Crate | Candidate Make targets |
| --- | --- |
| basilisk-stubs | `make mutation-run-crate-stubs` |
| basilisk-db | `make mutation-run-crate-db` |
| basilisk-config | `make mutation-run-crate-config` |
| basilisk-parser | `make mutation-run-crate-parser` |
| basilisk-mojo | `make mutation-run-crate-mojo` |
| basilisk-checker | `make mutation-run-checker-group-01` through `make mutation-run-checker-group-14`, plus `make mutation-run-checker-rule MUTATION_RULE=eNNNN` |
| basilisk-resolver | `make mutation-run-crate-resolver` |
