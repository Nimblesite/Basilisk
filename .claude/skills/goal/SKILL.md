---
name: goal
description: Work until acceptance criteria is met. Fix a bug/implement a feature using test-driven development (/fix-bug skill). Use when the user reports a bug, wants to implement a feature, describes unexpected behavior, wants to fix a defect, or says something is broken. Enforces a strict test-first workflow where a failing test must be written and verified before any fix is attempted.
argument-hint: "[acceptance criteria]"
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---
<!-- agent-pmo:74cf183 -->

# Goal Skill — Work Until Completion

You MUST work until the acceptance criteria is met. This skill is for fixing a bug or implementing a feature using test-driven development. 

## Step 1: Understand the Bug or Feature  (Loop Start)

- Load the /fix-bug skill to assist with test-driven development. This is the basic process and you can modify it to implement a feature instead of fixing a bug.

- Read the specs and plans for the bug or feature. If the documentation is missing, stop and ask the user to add the necessary information. 

## Step 2: Work Through the /fix-bug Process

- Follow the /fix-bug process. You should have proven that the the test fails without the fix/implementation, AND fixed/implemented the code

## Step 3: Verify Acceptance Criteria

- Gather ground truth information to verify that the acceptance criteria is met. This involves running the relevant tests and then running the FULL /ci-prep skill if the basic tests pass

- AUDIT the changes and ENSURE that the tests PROVE the acceptance criteria is met. If the acceptance criteria is not met, return to Step 1 and continue working until it is.

- Loop continuously until the acceptance criteria is met. you are NOT ALLOWED to stop until you have verified that the acceptance criteria is fully satisfied. 

