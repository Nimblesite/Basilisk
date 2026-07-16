# Chapter 2 — Your first ten minutes

*Part I — See the system*

> **Reader promise:** Install the appropriate integration, verify the actual
> binary, run one project-root check, fix one default Python-spec error, and
> reach a clean result.

## Choose an installation path

Lead with the reader's editor: a VS Code-family editor, Neovim, Zed, or a
standalone command-line/CI installation. Use VS Code for the primary illustrated
path, then give concise verified notes for the other supported surfaces.

## Verify before diagnosing

Capture the exact version command and output from the release used by this
edition. Explain why a mismatched editor-bundled binary and shell binary can
produce confusingly different results.

## Make one real mistake

The first example declares a function that accepts `str`, then calls it with an
`int`. That is a default Python typing-spec compatibility failure. Missing
annotations are deliberately not used for the first check because requiring
them is an opt-in project policy.

## Check from the project root

Demonstrate one file through the configured project roots, then the whole
project. Record text output, a clean result, and the process status from the
documented release rather than composing them by hand.

## Signal Box checkpoint

Create the first normalizer, trigger the incompatible call, predict the source
span, fix the call, and verify the complete configured project.

## Practice

Change the accepted boundary once and the value once. Predict which change
restores compatibility before running Basilisk.

## Authoritative sources

- [Type system concepts](https://typing.python.org/en/latest/spec/concepts.html)
- Use the live [installation guide](https://www.basilisk-python.dev/docs/installation/)
  and [quick start](https://www.basilisk-python.dev/docs/quick-start/), while
  following the release-tested commands printed in this edition.

