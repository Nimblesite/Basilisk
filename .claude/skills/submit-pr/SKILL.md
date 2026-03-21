---
name: submit-pr
description: Submit a pull request following DataProvider project standards
---

# Submit Pull Request

Create a pull request following project requirements.

## Ensure All Files Are Pushed

If there are files that have not been committed or pushed, stop and tell the user you cannot continue. List the files/commmits that have not been committed/pushed.

## Format

Run fmt for all Rust crates, and any other formatters across the workspace. If there are resulting changes, commmit and push these changes before continuing

## Run Lints

Run clippy on the entire workspace. If there are any failures, crash out and tell the user you cannot continue. List the failures.

## Get Context

Get the diff between main and current branch:

```bash
git diff main...HEAD
```

DO NOT include commit messages or branch names in analysis.

Read the PR template:

```bash
cat .github/PULL_REQUEST_TEMPLATE.md
```

## CI Check

Make sure the CI gh action and test.sh is including all tests from all crates/projects

## Sanity Check

Make sure nothing seems obviously wrong

## Website Check

Make sure the website is up to date with the latest changes

## Write PR Description

The template has three sections (gh will auto-populate structure):

### TLDR
- Few lines maximum
- Bullet points if many changes
- For people who won't read details

### Brief Details
- Keep BRIEF
- May reference code/files
- What changed and why

### How Do The Tests Prove This Works? (CRITICAL)
- Point to specific test files/methods
- Explain WHAT each test verifies
- Show HOW tests prove correctness, not just "tests added"

## Requirements

- TIGHT - no fluff
- ACCURATE - based on actual diff

## Submit

```bash
gh pr create
```
