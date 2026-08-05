# Release accuracy notes

These are editorial and implementation notes, not reader-facing manuscript.
The living edition currently targets Basilisk **0.39.0** at source commit
`b8ae454cfabc54d26d7e4efc029f2f01bd083bc8`, with bundled typeshed commit
`83c2518a9e6abbda0c44592c3483de459198f887`.

The official macOS arm64 release archive was checked on 2026-08-05. Its SHA-256
was `71f16a1ba02d1e1f99c72d2253fc8fbd2a194a3ca93eac3baf899593900cfc68`,
matching the published checksum. The extracted binary reported `basilisk
0.39.0` and Ruff `0.15.17`.

## Material deliberately excluded from Chapters 8 and 9

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

## Existing chapter audit backlog

- Chapters 2 and 3, and Chapters 10–12, remain outlines rather than finished
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
