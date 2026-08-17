# Contributing to Basilisk

**Basilisk is unlisted and is not accepting contributions.**

Basilisk's type checker was producing incorrect results. We asked for it to be removed from the `python/typing` conformance results, and it has been removed ([python/typing#2330](https://github.com/python/typing/pull/2330)). The code responsible is not isolated to a known set of rules, so we cannot say how many rules are affected. A code-quality tool that does not produce correct results is worse than useless.

The full statement: [www.basilisk-python.dev](https://www.basilisk-python.dev/). The author's public account: [an apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

## What that means for a pull request

**No fix to the type checker will be merged** — not a rule, not a diagnostic, not a false positive. The problem is not a list of bugs waiting for patches; it is that we cannot say which results were ever trustworthy. Repairing individual rules would produce a checker that is wrong in ways nobody has enumerated, and shipping that again is the thing we are stopping.

There is nothing to contribute to here yet. What comes next is a new product, rebuilt from the ground up, shipping only what can be trusted. That most likely will not include type checking. If type checking ever returns, it will be externally audited before release.

## What this repository is now

The record. It stays public because taking it down would erase what happened.

- [`docs/CONFORMANCE-INTEGRITY-AUDIT.md`](docs/CONFORMANCE-INTEGRITY-AUDIT.md) — how checker logic came to be fitted to the conformance fixtures, and how it went unnoticed.
- [`docs/specs/`](docs/specs/) and [`docs/plans/`](docs/plans/) — what was specified and what was built. They describe a product that is withdrawn; they are kept as evidence, not as promises.
- [`docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md`](docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md) — every word Basilisk says publicly, and the single source each surface copies from.
- [`delist/`](delist/README.md) — the unlisting runbook.

## If you found something wrong in the record

That is worth an issue. Corrections to the audit, the specs, or the statement — anywhere the account of what happened is inaccurate or incomplete — are welcome, and they are the only changes being reviewed.

Report a security issue in the usual private channel rather than an issue: [SECURITY.md](SECURITY.md).
