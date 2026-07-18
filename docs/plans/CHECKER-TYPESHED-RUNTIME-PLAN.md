# Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Pinned typing authority**: [`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This supplies the real standard-library `.pyi` bodies missing in [#324](https://github.com/Nimblesite/Basilisk/issues/324), so [#289](https://github.com/Nimblesite/Basilisk/issues/289) and [#288](https://github.com/Nimblesite/Basilisk/issues/288) can be fixed — offline and online alike — without changing the typing specification's resolution order.

## Contract {#TYPESHEDRT-MODEL}

Pinned step 3 says **“Typeshed stubs for the standard library”**, says those stubs are **“usually”** vendored, and says a provided custom path **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). “Usually” mandates neither bundling nor any Git policy; the transport below is Basilisk policy where the specification is silent.

Basilisk activates exactly one step-3 source, in precedence order:

1. `typeshed-path` — a user's canonical custom tree (bypasses everything else);
2. an exact `typeshed-commit` archive, or the latest `python/typeshed@main` archive resolved for this run/session — **downloaded over HTTPS, never `git clone`** — extracted and integrity-verified;
3. the **bundled full `stdlib/` ZIP snapshot** (real `.pyi` bodies) as the offline floor.

The **freshness-over-determinism** principle governs the default: no configuration resolves the *latest* `main`; a `typeshed-commit` pin (or **Pin current**) is determinism one line away. There is no refresh TTL, no automatic stale-checkout fallback, no fixed Python default, and no Python-version-to-commit map. An explicit commit is a deliberate immutable user selection, never automatic staleness. Custom or downloaded data wholly bypasses the bundled snapshot and its compiled lookups; the two never mix.

## Work {#TYPESHEDRT-WORK}

The pinned order puts standard-library typeshed at step 3, stub packages at step 4, inline `py.typed` packages at step 5, and optional vendored third-party stubs last ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Implement only what preserves that order:

1. **One selected source feeds step 3.** A single identity supplies module names, real `stdlib/*.pyi` bodies, `stdlib/VERSIONS`, and the derived `types-<distribution>` map. The compiled name index only *accelerates* the bundled snapshot; it never substitutes for a body.
2. **`acquire(config) -> Custom | Downloaded | Bundled`.** Return `Custom` immediately for a valid `typeshed-path`. Otherwise resolve one commit SHA — recording the **tree SHA** it points to from the same trusted metadata response — download the archive (default GitHub codeload, or the `typeshed-url` template for a known SHA), extract under always-on safety guards (reject `..`/absolute/symlink-escape entries and enforce entry-count + decompressed-size ceilings), integrity-verify the extracted tree against the recorded tree SHA unless verification is waived, then **atomically activate** one validated SHA. On *unpinned* failure return `Bundled`, never an older checkout; a validated tree matching an explicit pin stays eligible with no network.
3. **Cache and flags.** `typeshed-cache-path` relocates the extracted cache; `--no-typeshed-cache` downloads, verifies, then discards (hermetic); `typeshed-verify = false` / `--no-typeshed-verification` waives only the content-hash check (extraction safety stays on) and marks the source `UNVERIFIED`. A `typeshed-path` miss leaves step 3 and proceeds to step 4.
4. **Report and fingerprint.** Gate first analysis on acquisition; fingerprint checker caches with the source identity (SHA / custom-tree identity / bundled identity); and surface three orthogonal signals — **not pinned** (warning; fires in Latest mode *even on the bundled snapshot*, remedied by **Pin current**), **bundled-ZIP fallback** (high-severity, persistent), and **`UNVERIFIED`** (high-severity) — on CLI, LSP Service Info, and MCP ([§STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

Each checkbox is an independent automated test. The pinned specification says type checkers **“SHOULD resolve modules containing type information”** in its listed order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)); the acquisition mechanics below are Basilisk policy where that specification is silent.

### Source acquisition and identity {#TYPESHEDRT-ACCEPTANCE-SOURCE}

Step 3 requires **“Typeshed stubs for the standard library”**, while the same pinned text does not prescribe transport, cache age, or commit selection ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **Never clones.** Instrument process spawns and network calls; assert acquisition issues an HTTPS archive fetch and **never** invokes `git`, `git clone`, or a Git transport.
- [ ] **Current unpinned `main`:** seed cached SHA `A`, advertise remote SHA `B`, acquire, and assert the reported SHA, `.pyi` path, `stdlib/VERSIONS`, module names, and distribution map all come from `B`.
- [ ] **One generation:** give `A`, `B`, and the bundled snapshot conflicting sentinels; after activating `B`, assert every name/body/distribution lookup reads `B` and no fallback lookup occurs.
- [ ] **Failed update → bundled floor with real bodies:** seed `A`, fail remote resolution/verification, and assert the **bundled ZIP** and the exact persistent fallback warning are selected; `A` is never activated, no `.pyi` body is fabricated, and a bundled-body hover (`str.join`) still resolves offline.
- [ ] **Invalid checkout:** interrupt before promotion and separately corrupt the tree; assert no partial tree activates, then assert a successful retry atomically activates one complete SHA.
- [ ] **Extraction safety:** feed archives containing `..`/absolute paths, an escaping symlink, an over-count entry list, and a zip bomb; assert each is rejected **even with `--no-typeshed-verification`**, and nothing is written outside the cache root.
- [ ] **Concurrent callers:** acquire from CLI- and LSP-shaped callers against one cache; assert one complete source identity, one promotion, and no observation of a temporary tree.
- [ ] **Cache fingerprint:** different SHAs, custom-tree identities, and bundled identity miss the checker cache; identical identities hit it.

### Integrity, verification, and cache {#TYPESHEDRT-ACCEPTANCE-VERIFY}

GitHub archive tarball bytes are not stable, and a commit SHA also hashes commit metadata, so verification binds to the **tree** SHA the commit points to ([§STUBRES-TYPESHED-ACQUIRE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE)).

- [ ] **Tree-SHA binding:** verify against the recorded tree SHA; assert a tree whose bytes differ but content matches passes, and any content mutation fails — the raw tarball checksum is never the gate.
- [ ] **Re-fetch equivalence:** evict a pinned cache entry, re-download, and assert the extracted tree hashes to the same tree SHA even if the archive bytes differ; the pin never “expires”.
- [ ] **`--no-typeshed-cache`:** assert a fresh download is verified then discarded, leaving no cache entry, and the result equals the cached path's result.
- [ ] **Verification waived:** with `typeshed-verify = false` / `--no-typeshed-verification`, assert the hash check is skipped, extraction safety still runs, and the source reports `UNVERIFIED` on CLI, LSP, and MCP.
- [ ] **Alternate URL is archive-only:** point `typeshed-url` at a mirror; assert a *pinned* SHA fetches through it and verifies, while *Latest* with GitHub metadata unreachable cannot resolve a SHA and falls to the bundled floor.

### Explicit user sources {#TYPESHEDRT-ACCEPTANCE-OVERRIDES}

Pinned step 3 says a supplied custom typeshed **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **Exact commit:** configure SHA `A`; assert the active tree hashes to `A`'s tree SHA, a local mutation fails verification, later movement of `main` changes nothing, and the target Python never rewrites the pin.
- [ ] **Pinned reuse:** validate `A`, remove the network, and assert that exact immutable checkout remains eligible and reports `pinned`, never `stale`.
- [ ] **Pin current:** in Latest mode, resolve `main` to `B`, invoke **Pin current**, and assert `typeshed-commit` is written to `B`; repeat offline and assert it writes the *bundled snapshot* SHA.
- [ ] **Not-pinned advisory:** in Latest mode assert the warning-level *not pinned* advisory fires when a fresh `main` was downloaded **and** when the bundled snapshot supplied step 3; assert a `typeshed-path` or a set `typeshed-commit` raises no advisory.
- [ ] **Custom tree:** configure conflicting custom, download, and bundled signatures; assert the resolved custom path is used verbatim, custom wins, and acquisition plus all other step-3 lookups are bypassed.
- [ ] **Custom miss:** omit `X` only from custom while putting it in download/bundled; assert resolution goes directly to step 4 and never rescues `X` from another step-3 source.
- [ ] **Path validation:** cover absolute/workspace-relative paths, required top-level `stdlib/`, nonexistent paths, malformed trees, and deterministic diagnostics.

### Python target semantics {#TYPESHEDRT-ACCEPTANCE-TARGET}

The pinned stub specification says checkers should fully support **“Simple version and platform checks”**; its directives say checkers are **“expected to understand simple version and platform checks”** using `sys.version_info` and `sys.platform` ([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst), [directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst), both `python/typing@6ef9f77`).

- [ ] **Same SHA, different targets:** run two target versions against one SHA; assert acquisition identity is unchanged while `stdlib/VERSIONS` admits the fixture's target-specific modules.
- [ ] **Guard selection:** put incompatible declarations behind simple version/platform guards; assert only the matching branch reaches symbols, hover, completion, and diagnostics — never a union of branches.
- [ ] **No commit inference:** instrument the network and change only `python-version`/`python-platform`; assert no different SHA is selected, guessed, or fetched.
- [ ] **No manufactured target:** assert configuration, generated data, and bundled data contain no Python-version-to-SHA map and no fixed Python target appears without project/interpreter evidence.

### Resolution and stub semantics {#TYPESHEDRT-ACCEPTANCE-RESOLUTION}

The pinned specification orders manual stubs, user code, stdlib typeshed, stub packages, inline `py.typed`, and optional vendored third-party stubs; it also says checkers **“MUST maintain the normal resolution order of checking `*.pyi` before `*.py` files”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **Six steps:** collide module `X` at every step, remove each winner in turn, and assert `1 → 2 → 3 → 4 → 5 → 6 → unresolved`, matching the retained diagram.
- [ ] **Stub package versus inline:** install `foopkg-stubs` beside inline `py.typed` `foopkg`; assert step 4 wins over step 5.
- [ ] **Partial stub package:** add `partial\n` to `foopkg-stubs/py.typed`; assert missing modules merge/fall through to steps 5/6 exactly as the pinned partial-stub clauses require.
- [ ] **`.pyi` precedence:** place `.pyi` and `.py` for one module at the winning location; assert only `.pyi` supplies the public interface.
- [ ] **Public interface:** cover redundant aliases, all specified `__all__` mutations, relative and absolute star imports, import cycles, and target-selected re-exports.

### #288 and #289 behavior {#TYPESHEDRT-ACCEPTANCE-HOVER}

Pinned stub rules require class methods, function/method definitions, imports, aliases, typing features, decorators, and cycles to be understood ([`python/typing@6ef9f77`, “Supported Constructs”, “Classes”, “Functions and Methods”](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **#289:** resolve `unittest.mock.Mock` from the active source; assert hover reaches the class and the constructor resolved through the canonical chain (metaclass `__call__`, then `__new__`/`__init__`, following inheritance) via [CHKARCH-DIAG-CTOR-CALLABLE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CTOR-CALLABLE), with typeshed provenance — not a bare local `__init__` and not merely import text.
- [ ] **#288:** resolve `str.join` from `stdlib/builtins.pyi`; assert bound-method hover preserves the full `@overload` set (`LiteralString` and generic overloads never collapsed); mutate only the fixture signature and assert hover changes, proving no hand-maintained table won.
- [ ] **Offline parity:** repeat #288/#289 on the **bundled ZIP** (network removed) and assert identical real-body signatures — the offline floor is not names-only.
- [ ] **Override behavior:** repeat both with conflicting custom stubs and assert custom signatures/provenance.
- [ ] **Shared declaration:** assert hover, signature help, completion, and go-to-definition use the same indexed declaration and source identity.

### Licensing and release gates {#TYPESHEDRT-ACCEPTANCE-GATES}

The bundled ZIP redistributes typeshed source, so Apache 2.0 §4 attaches; downloads are fetched by the user and are not Basilisk redistribution ([§STUBRES-TYPESHED-LICENSE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-LICENSE)). The full pinned quotation and retained diagram in [STUBRES-PEP561](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561) remain the local audit surface; the maintained typing specification and conformance suite remain the upstream authority.

- [ ] **License bytes:** assert the bundled ZIP ships typeshed's composite `LICENSE` **byte-for-byte identical** to the file at the bundled SHA.
- [ ] **Conditional NOTICE:** assert `NOTICE` presence/absence matches the bundled SHA (copied iff present); the reviewed SHA `83c2518a9e6abbda0c44592c3483de459198f887` has none.
- [ ] **Attribution:** assert `THIRD-PARTY-LICENSES` carries the composite Apache-2.0/MIT text and `NOTICES` names typeshed, both licenses, and the exact bundled SHA.
- [ ] **Human-review gate:** if the bundled SHA changes the `LICENSE` text or license identity, or adds a `NOTICE`, assert the release **fails for human review** before packaging.
- [ ] **Conformance:** run a freshly cloned, unmodified `python/typing@main` conformance harness against the clean release binary; require 100% and zero false positives.
- [ ] **Docs integrity:** validate Mermaid rendering, anchors, links, and the full `6ef9f7719ecfff09dad8724ef42b621fd994fb5e` pin in every touched typeshed section.
- [ ] **No forbidden policy:** reject documentation containing a refresh TTL, automatic stale-checkout fallback, Python-version-to-SHA map, fixed Python default, or any `git clone` transport; permit the explicit user commit pin and custom path.
