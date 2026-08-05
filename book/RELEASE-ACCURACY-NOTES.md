# Release accuracy notes

These are editorial and implementation notes, not reader-facing manuscript.
The living edition currently targets Basilisk **0.39.0** at source commit
`b8ae454cfabc54d26d7e4efc029f2f01bd083bc8`, with bundled typeshed commit
`83c2518a9e6abbda0c44592c3483de459198f887`.

The official macOS arm64 release archive was checked on 2026-08-05. Its SHA-256
was `71f16a1ba02d1e1f99c72d2253fc8fbd2a194a3ca93eac3baf899593900cfc68`,
matching the published checksum. The extracted binary reported `basilisk
0.39.0` and Ruff `0.15.17`.

## Material deliberately excluded from Chapters 8–10

- The current branch's new bidirectional type-inference engine is not treated
  as released behavior.
- The branch adds `typeshed-package`, wheel-SHA pinning, and uv lockfile
  auto-detection as a third typeshed source. Basilisk 0.39.0 has only the
  bundled/pinned-commit and custom-folder sources, so Chapter 8 describes only
  those two.
- Working-tree performance changes to embedded typeshed indexing and archive
  activation are not used as book claims.
- The old Chapter 9 screenshots came from an unreleased placeholder build and
  showed obsolete rule codes, counts, controls, and path selectors. They were
  replaced on 2026-08-05 by direct captures driven through the v0.39.0 source
  tag with the official v0.39.0 macOS arm64 binaries. The Configuration Editor
  JavaScript used for capture matched the published v0.39.0 VSIX byte for byte;
  that VSIX's SHA-256 was
  `74ef14d9e4e87469eb59c2493cfad16545ee49333c321e8672317fc8c010502e`,
  matching the published checksum.
- Chapter 10 does not describe a fix preview or dry run: v0.39.0 `fix` writes
  immediately. It does not call the default tier universally runtime-safe,
  treat an `Any` insertion as inferred domain knowledge, describe adoption as
  file-specific, or claim that status calculates coverage.
- The two Chapter 10 terminal figures were captured on 2026-08-05 by executing
  the real commands in a headed isolated VS Code 1.131.0 integrated terminal
  against the checksum-verified official v0.39.0 VSIX. Their untouched
  2880×1800 masters and hashes are recorded in `figures.json`; the publication
  copies are uniform full-frame resizes with no repainted or composited pixels.

## Specification and release gaps

These items must not be promoted into reader-facing product claims until the
specification, implementation, and tests agree.

1. **Spec-ID structure blocks a clean spec audit.** Behavioral headings in
   [`CHECKER-STUB-RESOLUTION-SPEC.md`](../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md)
   use Pandoc-style `{#STUBRES-...}` anchors instead of the repository's required
   bracket IDs. The `spec-check` structural gate therefore stops before it can
   claim full spec-to-code coverage.
2. **Hybrid generation fallback is not per function.**
   `STUBRES-AUTOGEN-MODES` says hybrid generation falls back to AST per
   function. In 0.39.0,
   [`hybrid.rs`](../crates/basilisk-stubs/src/generate/hybrid.rs) keeps the
   entire runtime result when module introspection succeeds and uses the entire
   AST result only when that attempt fails.
3. **Runtime generation loses signature details.** The 0.39.0 generator
   observes parameter kinds and defaults but its formatted output does not
   preserve the keyword-only separator or default in the verified
   `vendor_sensor` example. The release produced
   `def fetch_packet(sensor_id, timeout) -> Any` for a runtime signature whose
   second parameter is keyword-only and defaulted. This is why Chapter 8 calls
   generated output best-effort and requires review.
4. **Third-party stub provenance can be mislabeled.** The provenance model maps
   a trusted external stub package to the same Tier 1 value whose display label
   is `typeshed`. `STUBRES-PROVENANCE-DIAG` and
   `STUBRES-PROVENANCE-HOVER` should distinguish the package source or the
   implementation should stop presenting that label as typeshed-specific.
5. **`stubs status` does not calculate coverage.** `STUBRES-AUTOGEN` says the
   command reports coverage, while the 0.39.0 command lists generated `.pyi`
   files without comparing them with untyped imports. Treat it as an inventory,
   not a coverage report.
6. **Tag-editor mutation wording is stale.**
   `CHKTAG-CONFIGURATION-EDITOR` describes tag-selector expansion into explicit
   per-rule entries. The released wire model has distinct `SetTag` and
   `SetRule` mutations; `SetTag` persists one `[tool.basilisk.rule-tags]` line,
   while selectors are read-side occurrence queries. Chapter 9 follows the
   released model and does not claim selector-based mutation.
7. **Mass-autofix spec IDs block the structural audit.** Every behavioral
   heading in `LSP-MASS-AUTOFIX-SPEC.md` at v0.39.0 uses a Pandoc-style
   `{#AUTOFIX-...}` anchor rather than the bracketed requirement IDs required
   by the `spec-check` skill. Its structural gate stopped before a full
   spec-to-code coverage result could be claimed; Chapter 10 therefore uses a
   manual exact-release audit plus executable evidence.
8. **The default fix tier can produce an unresolved runtime name.** The
   v0.39.0 BSK-0001, BSK-0002, and related fixers insert bare `Any` text but do
   not add an import. Importing such output without an existing `Any` binding
   raises `NameError` on the verified Python 3.12 and 3.13 runtimes. Under the
   `strictness` tag, the Chapter 10 result also becomes two BSK-0014 errors.
   The manuscript treats the tier label as a static rule allowlist, not a
   per-edit safety proof.
9. **Released website copy names the wrong return placeholder.** Several
   v0.39.0 website pages say the missing-return fix inserts `-> None`; the
   released implementation and binary insert `-> Any`. Chapter 10 and its real
   capture use `Any`.
10. **The configuration-editor fix boundary is inconsistent.** The v0.39.0
    mass-autofix specification says the Configuration Editor has no fix
    affordance of its own, while the released editor includes **Apply safe
    fixes**. Chapter 10 teaches the independently verified CLI workflow and
    makes no claim about that editor affordance.
11. **Analyze-rule adoption never selects `disabled`.** The adoption flow in
    the specification says analyze rules may be disabled. The v0.39.0 CLI and
    LSP implementations write `warning` for every adopted error code. The
    chapter describes only the observed warning representation.
12. **Graduation is implemented only by the CLI recomputation.** The
    specification says re-running adoption removes entries for rules that no
    longer fire. The v0.39.0 CLI does this; the LSP `adoptFile` and
    `adoptWorkspace` handlers add warning entries but do not remove stale ones.
    Chapter 10 explicitly scopes graduation instructions to the CLI.
13. **Warning entries have no adoption ownership.** `adopt --status` reports
    every ordinary warning rule entry, including deliberate policy, and
    `unadopt` deletes every such entry in the selected governing config. This
    follows the no-marker representation but makes status and removal less
    discriminating than their names suggest. The chapter warns readers to
    inspect the configuration diff and maintain a strict fallback.
14. **Released adoption summaries omit or overstate scope.** The editor's
    adopted-rule count includes only below-error PEP rules, omitting adopted
    Basilisk rules, while released copy says new violations still fail even
    though a folder-level warning entry also grades new same-rule violations to
    warning. Neither claim appears in Chapter 10.

## Existing chapter audit backlog

- Chapters 2 and 3, and Chapters 11–12, remain outlines rather than finished
  chapters.
- Chapters 0 and 1 still contain pre-0.39 command-scope and edition-evidence
  debt. In 0.39.0, `check` evaluates PEP typing rules and `analyze` evaluates
  configured opt-in policy; examples must not collapse them into one command.
- Chapters 4 and 5 contain several normative lessons that 0.39.0 does not
  reliably demonstrate. Exact inferred-type and clean-check claims need to be
  narrowed to the cases the release actually detects.
- Chapters 6 and 7 need runtime-version qualifications and several prose fixes,
  including `TypeIs` availability, `Required` inside `total=False` TypedDicts,
  declaration order in displayed snippets, and narrower claims about what a
  clean run proves.

Those chapters remain outside the publication gate until corrected. Their
status does not reduce Chapters 8 and 9 to drafts; it limits what a full-book
release build may claim.

## Living-edition release update

When Basilisk releases a new version, update these items as one review:

1. `book.json`, `metadata.yaml`, and the chapter evidence release fields;
2. the immutable release and source-spec URLs in `sources.json`;
3. the bundled typeshed pin in every completed checkpoint that uses it;
4. every exact command output, version string, diagnostic count, diagram label,
   screenshot, untouched capture master, and capture-provenance record tied to
   the old release;
5. the release artifact checksum and test results; and
6. this gap list, moving implemented items into reader prose only after the
   specification, released implementation, and executable evidence agree.
