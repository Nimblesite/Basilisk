<!-- Implements [README]. See docs/specs/DOCS-README-SPEC.md
     THIS IS THE ONLY AUTHORED README. Every published README — GitHub, the VS
     Code Marketplace / Open VSX, PyPI — is generated from this file by
     scripts/gen_readmes.py. Do not edit the generated copies.
     The statement itself is NOT authored here: `{{withdrawal:…}}` is substituted
     from docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md ([WITHDRAWAL-COPY]).
     Change the message there, never here.
     Exactly ONE paragraph may vary per target: the identity line below
     ([README-IDENTITY]). A second variant block is a review failure. -->
# {{withdrawal:title}}

<!--v:github-->
> **You are reading the Basilisk source repository** — the checker, language server, editor extensions, and website all live here.
<!--/v:github-->
<!--v:vscode-->
> **You are reading the Basilisk extension listing** for VS Code, Cursor, Windsurf, and every VS Code fork.
<!--/v:vscode-->
<!--v:pypi-->
> **You are reading the `basilisk-python` wheel listing** — the Basilisk CLI packaged for `pip`/`uv`.
<!--/v:pypi-->
<!--v:zed-->
> **You are reading the Basilisk Zed extension listing.**
<!--/v:zed-->
<!--v:nvim-->
> **You are reading the `basilisk.nvim` plugin listing.**
<!--/v:nvim-->

{{withdrawal:full}}

## What to do now

{{withdrawal:action}}

## Acknowledgments

Basilisk is built on [Ruff](https://github.com/astral-sh/ruff) by [Astral](https://astral.sh/), whose parser, AST, and formatter crates it embeds (MIT), and on standard-library type stubs from [typeshed](https://github.com/python/typeshed) (Apache-2.0, with MIT-licensed parts). Neither project is responsible for how Basilisk used them. Full component list and required notices: [NOTICES](NOTICES) and [RUST-DEPENDENCY-LICENSES](RUST-DEPENDENCY-LICENSES).

## License

Basilisk source code is MIT licensed. Binary distributions also contain third-party components under the licenses shipped beside each artifact.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
