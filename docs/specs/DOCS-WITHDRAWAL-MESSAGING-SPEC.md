# Basilisk withdrawal — canonical messaging {#WITHDRAWAL}

Single source of truth for everything Basilisk says publicly. Every README, listing, website page, and the CLI's own output copies from here. No surface writes its own version. Where old copy conflicts, this wins.

## The message {#WITHDRAWAL-CLAIMS}

1. **Basilisk's type checker was producing incorrect results.**
2. **We asked for it to be removed from the `python/typing` results, and it was** — [python/typing#2330](https://github.com/python/typing/pull/2330).
3. **The code responsible is not isolated. We cannot say how many rules are affected.**
4. **A code-quality tool that does not produce correct results is worse than useless.**
5. **Remove Basilisk from your pipeline.** The type checker is being made inert, and every distribution channel is being unlisted.
6. **We are unlisting first, then rebuilding from the ground up as a new product** — not fixing, auditing, or salvaging this code.
7. **Nothing is relisted until it has been rebuilt from components we can vouch for.** It will ship only what can be trusted — most likely not type checking. If type checking returns, it will go through **external auditing** before being released.

Nothing else is asserted. One vocabulary, in this order: *incorrect results → removed from `python/typing` → unlisted from every channel → the CLI is inert → rebuilt from the ground up as a new product*. "Unlisted" throughout — never delisted, withdrawn, deprecated, or discontinued as synonyms.

## Copy {#WITHDRAWAL-COPY}

Verbatim. Doesn't fit? Use the shorter one.

**One line** {#WITHDRAWAL-COPY-LINE} — repo description, package summaries, store descriptions, OG tags:

> Basilisk's type checker produced incorrect results. Basilisk is unlisted and is being rebuilt from the ground up as a new product.

**Short** {#WITHDRAWAL-COPY-SHORT} — READMEs, PyPI, extension listings:

> **Basilisk's type checker was producing incorrect results. Basilisk is unlisted everywhere.**
>
> We asked for Basilisk to be removed from the `python/typing` conformance results, and it has been removed ([python/typing#2330](https://github.com/python/typing/pull/2330)). The code responsible is not isolated to a known set of rules, so we cannot say how many rules are affected. A code-quality tool that does not produce correct results is worse than useless.
>
> **Remove Basilisk from your pipeline.** Every distribution channel is being unlisted, and the type checker is inert — it checks nothing and exits non-zero, so a build that still calls it fails loudly instead of reporting a clean run.
>
> What comes next is a new product, rebuilt from the ground up, shipping only what can be trusted. That most likely will not include type checking. Nothing is relisted until it has been rebuilt from components we can vouch for.
>
> Basilisk's author has published a full public account: [an apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

**What to do now** {#WITHDRAWAL-COPY-ACTION} — every README and store listing carries this under the statement. It is the only part of the message that asks the reader to do something, so it never gets cut for length:

> **Remove Basilisk from your pipeline, your pre-commit hooks, and your editor.** Uninstall the CLI and the extension.
>
> The type checker is inert: it checks nothing, and every invocation fails. It prints this statement and exits non-zero, so a build that still calls it fails loudly rather than reporting a clean run. Do not treat that failure as a finding about your code.
>
> **Treat every result Basilisk gave you as unverified.** A clean run was never evidence that your code was clean, and an error it reported may never have been real.
>
> Every distribution channel is being unlisted. Nothing will be relisted until it has been rebuilt from components we can vouch for.

**Full** {#WITHDRAWAL-COPY-FULL} — website home and README body. There is no longer form:

> # Basilisk is unlisted
>
> **Basilisk's type checker was producing incorrect results.** Rules decided from the way code was *spelled* rather than what it meant, so they could be wrong in both directions — a false error on correct code, or silence on a real bug.
>
> **We asked for Basilisk to be removed from the `python/typing` conformance results, and it has been removed** ([python/typing#2330](https://github.com/python/typing/pull/2330)). That score did not demonstrate correctness.
>
> **We cannot tell you how much of the checker this affects.** The code responsible is not isolated to a known set of rules. We will not estimate. That uncertainty is the reason for everything below.
>
> **A code-quality tool that does not produce correct results is worse than useless.** Basilisk is being unlisted everywhere it was published — the VS Code Marketplace, Open VSX, the Zed registry, PyPI, the Homebrew tap, and the Scoop bucket — and the type checker is inert. Remove it from your pipeline; it checks nothing, and every invocation fails rather than reporting a clean run.
>
> **We are not fixing Basilisk's type checker code. We are rebuilding from the ground up as a new product.** It will ship only what can be trusted. That most likely will not include type checking. Nothing is relisted until it has been rebuilt from components we can vouch for. If type checking ever returns, it will be externally audited before release.
>
> Basilisk's author has published a full public account: [an apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

## Never {#WITHDRAWAL-PROHIBITED}

- **Never quote the apology** — link it, neutrally, nowhere else. It speaks for itself in its author's words.
- **No conformance or benchmark figure**, in any tense, caveated or archived.
- **No feature marketing, rule counts, or per-rule docs** — including for parts that never touched the checker.
- **No scoping reassurance** — never "only a few rules", "the language server is fine, keep using it". Claim 3 forbids it.
- **No blame outside the project.** No timeline. No install instructions.

Tone: plain declaratives, active voice, worst part first. One statement of fault, then facts. No hedging, no repeated apology. Under a minute to read.

## Unlisting {#WITHDRAWAL-UNLIST}

Marketplace, Open VSX, Zed registry, PyPI, Homebrew tap, Scoop bucket — all unlisted. The repo stays public with the [full copy](#WITHDRAWAL-COPY-FULL) as its README — taking it down would erase what happened. Installed copies aren't force-removed; they go inert.

**One last release, then no more.** Unlisting hides the listing; it does not touch the copy already installed on a developer's machine. So exactly one final version ships to every channel first — carrying this statement and the [inert CLI](#WITHDRAWAL-INERT) — and the channel is unlisted immediately after. That is the only way an existing install learns what happened. Existing GitHub Releases stay (deleting them destroys the record); after the final one, no new releases. Order per channel, no exceptions: **publish the final version → verify it is live → unlist.** The runbook and its scripts: [`delist/`](../../delist/README.md).

Sources to edit, never the generated artefact: READMEs come from [`docs/readme/*.src.md`](../readme/); the website collapses to one notice page, with every retired URL — `/docs/*`, `/blog/*`, `/errors/BSK-XXXX/` — **redirecting** to it so links from installed binaries and search results land on the explanation rather than a 404 or a second copy of the statement. Internal specs, plans, and the [integrity audit](../CONFORMANCE-INTEGRITY-AUDIT.md) are not marketing surfaces — they are the record. Keep them, marked superseded.

## Surfaces {#WITHDRAWAL-SURFACES}

Every surface below carries a block from [Copy](#WITHDRAWAL-COPY) and nothing else. None writes its own version; each is generated or copied from this file.

| Surface | Block | Generated by |
|---|---|---|
| Website home | [full](#WITHDRAWAL-COPY-FULL) | `scripts/gen_withdrawal_copy.py` → `website/src/_data/withdrawal.json` |
| Every retired website URL | redirect to `/` | `website/src/notice.njk` |
| GitHub / VSIX / PyPI / Zed / Neovim READMEs | [full](#WITHDRAWAL-COPY-FULL) + [action](#WITHDRAWAL-COPY-ACTION) | `scripts/gen_readmes.py` from `docs/readme/README.src.md` |
| Package + store description fields | [one line](#WITHDRAWAL-COPY-LINE) | copied by hand, asserted by `scripts/test_published_readmes.py` |
| CLI, every invocation | [notice](#WITHDRAWAL-INERT-TEXT) | `crates/basilisk-cli/src/main.rs` |
| VS Code extension | [notice](#WITHDRAWAL-INERT-TEXT) | `vscode-extension/src/extension.ts` |

The extension ships **no checker binary and no type-checking UI** — no diagnostics, commands, views, settings, debugger, or profiler. It activates, states this, and links the website.

## Inert Type Checker CLI {#WITHDRAWAL-INERT}

**Every invocation fails.** Bare `basilisk`, every subcommand, every flag, `--help`, a bad argument: print the notice to **stderr** and exit `4` (*unlisted*, added to [CHKARCH-CLI-EXITCODES](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-EXITCODES)). No parsing, no analysis, no file touched, no server. Stdout emits nothing ever, so `--output json > report.json` yields an empty file, not prose a consumer might parse. Never exit `0` — a pipeline that still calls Basilisk must break, loudly, rather than read a clean run into it — and never `1`, because "errors found" would be one more incorrect result. `--version` is the sole exception, exit `0`: package managers and installed extensions verify against it and would otherwise hang instead of showing the notice.

Exact text {#WITHDRAWAL-INERT-TEXT}, no colour or emoji:

```text
Basilisk is unlisted. Its type checker is inert and checks nothing.

Basilisk's type checker was producing incorrect results. The code responsible is not isolated to a known set of rules, so we cannot say how many rules are affected. We asked for Basilisk to be removed from the python/typing conformance results, and it has been removed: https://github.com/python/typing/pull/2330

A code-quality tool that does not produce correct results is worse than useless. Remove Basilisk from your pipeline, your pre-commit hooks, and your editor. This command failed on purpose. It is not a finding about your code.

We are not fixing this code. We are rebuilding from the ground up as a new product, shipping only what can be trusted. If type checking ever returns, it will be externally audited before release.

A full public account: https://www.christianfindlay.com/blog/basilisk-conformance-apology
```
