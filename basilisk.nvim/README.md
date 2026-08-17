<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
# Basilisk is unlisted

> **You are reading the `basilisk.nvim` plugin listing.**

**Basilisk's type checker was producing incorrect results.** Rules decided from the way code was *spelled* rather than what it meant, so they could be wrong in both directions — a false error on correct code, or silence on a real bug.

**We asked for Basilisk to be removed from the `python/typing` conformance results, and it has been removed** ([python/typing#2330](https://github.com/python/typing/pull/2330)). That score did not demonstrate correctness.

**We cannot tell you how much of the checker this affects.** The code responsible is not isolated to a known set of rules. We will not estimate. That uncertainty is the reason for everything below.

**A code-quality tool that does not produce correct results is worse than useless.** Basilisk is being unlisted everywhere it was published — the VS Code Marketplace, Open VSX, the Zed registry, PyPI, the Homebrew tap, and the Scoop bucket — and the type checker is inert. Remove it from your pipeline; it checks nothing, and every invocation fails rather than reporting a clean run.

**We are not fixing Basilisk's type checker code. We are rebuilding from the ground up as a new product.** It will ship only what can be trusted. That most likely will not include type checking. Nothing is relisted until it has been rebuilt from components we can vouch for. If type checking ever returns, it will be externally audited before release.

Basilisk's author has published a full public account: [an apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

## What to do now

**Remove Basilisk from your pipeline, your pre-commit hooks, and your editor.** Uninstall the CLI and the extension.

The type checker is inert: it checks nothing, and every invocation fails. It prints this statement and exits non-zero, so a build that still calls it fails loudly rather than reporting a clean run. Do not treat that failure as a finding about your code.

**Treat every result Basilisk gave you as unverified.** A clean run was never evidence that your code was clean, and an error it reported may never have been real.

Every distribution channel is being unlisted. Nothing will be relisted until it has been rebuilt from components we can vouch for.

## Acknowledgments

Basilisk is built on [Ruff](https://github.com/astral-sh/ruff) by [Astral](https://astral.sh/), whose parser, AST, and formatter crates it embeds (MIT), and on standard-library type stubs from [typeshed](https://github.com/python/typeshed) (Apache-2.0, with MIT-licensed parts). Neither project is responsible for how Basilisk used them. Full component list and required notices: [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES) and [RUST-DEPENDENCY-LICENSES](https://github.com/Nimblesite/Basilisk/blob/main/RUST-DEPENDENCY-LICENSES).

## License

Basilisk source code is MIT licensed. Binary distributions also contain third-party components under the licenses shipped beside each artifact.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
