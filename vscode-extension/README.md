<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
# Basilisk is unlisted

> **You are reading the Basilisk extension listing** for VS Code, Cursor, Windsurf, and every VS Code fork.

**Basilisk's type checker was producing incorrect results.** Rules decided from the way code was *spelled* rather than what it meant, so they could be wrong in both directions — a false error on correct code, or silence on a real bug.

**We asked for Basilisk to be removed from the `python/typing` conformance results, and it has been removed.** The score it held was not evidence of anything.

**We cannot tell you how much of the checker this affects.** The code responsible is not isolated to a known set of rules. We will not estimate. That uncertainty is the reason for everything below.

**A code-quality tool that does not produce correct results is worse than useless.** Basilisk is being delisted everywhere it was published — VS Code Marketplace, Open VSX, PyPI, Homebrew, Scoop — and the type checker is being made inert. Remove it from your pipeline; it checks nothing, and it fails rather than reporting a clean run.

**We are not fixing Basilisk's type checker code. We are rebuilding Basilisk.** What comes next is a new product, built from the ground up, shipping only what can be shown to be trustworthy. That most likely will not include type checking. Nothing is relisted until it has been rebuilt from components we can vouch for. If type checking ever returns, it will come from established third-party engines, or code audited by a third party.

Basilisk's author has published a full public account: [an apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

## What to do now

**Remove Basilisk from your pipeline, your pre-commit hooks, and your editor.** Uninstall the CLI and the extension.

The type checker is being made inert: it checks nothing, and it exits non-zero so a build that still calls it fails loudly rather than reporting a clean run. Do not treat that failure as a finding about your code.

**Treat every result Basilisk gave you as unverified.** A clean run was never evidence that your code was clean, and an error it reported may never have been real.

Every distribution channel is being delisted. Nothing will be relisted until it has been rebuilt from components we can vouch for.

## Acknowledgments

Basilisk is built on [Ruff](https://github.com/astral-sh/ruff) by [Astral](https://astral.sh/), whose parser, AST, and formatter crates it embeds (MIT), and on standard-library type stubs from [typeshed](https://github.com/python/typeshed) (Apache-2.0, with MIT-licensed parts). Neither project is responsible for how Basilisk used them. Full component list and required notices: [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES) and [RUST-DEPENDENCY-LICENSES](https://github.com/Nimblesite/Basilisk/blob/main/RUST-DEPENDENCY-LICENSES).

## License

Basilisk source code is MIT licensed. Binary distributions also contain third-party components under the licenses shipped beside each artifact.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
