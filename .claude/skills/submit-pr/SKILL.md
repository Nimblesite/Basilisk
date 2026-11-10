---
name: submit-pr
description: Creates a pull request with a well-structured description after verifying CI passes. Use when the user asks to submit, create, or open a pull request.
---
<!-- agent-pmo:74cf183 -->

# Submit PR

Create a pull request for the current branch with a well-structured description.

## Steps

Reuse verified checks from this session only when the code and inputs they cover have not changed.

1. Discover the repository's actual CI gates from its instructions, workflows and build scripts. Run `make ci` when that target exists; otherwise run the configured commands directly, including their setup and services. A missing convenience target is a command-discovery problem, not a reason to stop or manufacture a passing target. For any failed check, inspect its logs, fix the root cause, rerun that check and affected checks, and continue until all required pre-PR checks pass. Never weaken assertions, suppress failures or claim an unavailable check passed. Only an unresolved dependency on credentials, permissions or an external service warrants reporting a blocker; complete independent preparation first.
2. **Generate the diff against main.** Run `git diff main...HEAD > /tmp/pr-diff.txt` to capture the full diff between the current branch and the head of main. This is the ONLY source of truth for what the PR contains. **Warning:** the diff can be very large. If the diff file exceeds context limits, process it in chunks (e.g., read sections with `head`/`tail` or split by file) rather than trying to load it all at once.
3. **Derive the PR title and description SOLELY from the diff.** Read the diff output and summarize what changed. Ignore commit messages, branch names, and any other metadata — only the actual code/content diff matters.
4. Use the repository's PR template when present. If no template exists, use the sections below directly; its absence does not block submission.
5. Fill in (based on the diff analysis from step 3):
   - TLDR: one sentence
   - What Was Added: new files, features, deps
   - What Was Changed/Deleted: modified behaviour
   - How Tests Prove It Works: specific test names or output
   - Spec/Doc Changes: if any
   - Breaking Changes: yes/no + description
6. Commit and push the verified changes to the user's current feature branch, preserving unrelated work. Use `gh pr create` with the body supplied through `--body-file`; if that branch already has an open PR, update it instead of creating a duplicate. Return the PR URL. Distinguish local check results from hosted CI; inspect hosted failures after creation, fix their causes and push repairs to the same branch.

## Rules

- A failed check starts the repair loop; it does not end the task. Create the PR after the repository's required pre-PR checks pass, whether through `make ci` or their configured individual commands.
- PR description must be specific and tight — no vague placeholders
- Link to the relevant GitHub issue if one exists

## Success criteria

- The actual required pre-PR checks passed, with commands and evidence recorded
- PR created with `gh pr create`, or the existing branch PR updated
- PR URL returned to user
