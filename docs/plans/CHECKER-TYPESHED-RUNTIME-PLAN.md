# [TYPESHEDRT-OVERVIEW] Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT-OVERVIEW}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Pinned typing authority**: [`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This supplies the real standard-library `.pyi` bodies missing in [#324](https://github.com/Nimblesite/Basilisk/issues/324), so [#289](https://github.com/Nimblesite/Basilisk/issues/289) and [#288](https://github.com/Nimblesite/Basilisk/issues/288) can be fixed — offline and online alike — without changing the typing specification's resolution order.

## [TYPESHEDRT-MODEL] Contract {#TYPESHEDRT-MODEL}

Pinned step 3 says **“Typeshed stubs for the standard library”**, says those stubs are **“usually”** vendored, and says a provided custom path **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). “Usually” mandates neither bundling nor any Git policy; the transport below is Basilisk policy where the specification is silent.

Basilisk activates one complete source, always already on this machine: a
**pinned commit** (the embedded bundle when the SHA is the bundled one, else that
commit's store entry) or a **custom folder**. Both fail closed. It never clones,
mixes sources, maps Python versions to commits, or changes an exact identity.
Custom is user-managed.

**The checker never downloads** ([§STUBRES-TYPESHED-OFFLINE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-OFFLINE)).
Acquisition is a segregated component a person invokes
([§STUBRES-TYPESHED-DOWNLOAD](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD));
**Download latest** acquires `main` and writes that SHA as the pin.

## [TYPESHEDRT-WORK] Work {#TYPESHEDRT-WORK}

The pinned order puts standard-library typeshed at step 3, stub packages at step 4, inline `py.typed` packages at step 5, and optional vendored third-party stubs last ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Implement only what preserves that order:

1. One identity supplies module names, `VERSIONS`, real `.pyi` bodies, and derived indexes.
2. **Segregate transport into its own crate** so the crate the checker links
   against carries no HTTP dependency and the analysis path cannot reach the
   network ([§TYPESHEDRT-SEGREGATION](#TYPESHEDRT-SEGREGATION)).
3. The download component resolves trusted commit→tree metadata, streams a safe
   archive through the safety, shape, approved-license/NOTICE, and Git-tree
   gates, reconstructs the commit object and asserts it hashes to the requested
   SHA, then dumps the accepted tree, that commit object, and a manifest into the
   store. Failure writes nothing.
4. The checker resolves a pin from the store or the bundle and verifies it
   offline by hashing; missing or corrupt fails hard. A custom miss proceeds to step 4.
5. Fingerprint caches by source identity, and return active source/full SHA plus
   composable `UNPINNED`, `LICENSE CHANGED`, `USER-MANAGED SOURCE`, and `NO SOURCE`
   statuses on CLI/LSP/MCP ([§STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).

## [TYPESHEDRT-SEGREGATION] Segregation {#TYPESHEDRT-SEGREGATION}

| Crate | May link an HTTP client | Role |
|---|---|---|
| `basilisk-stubs` | **no** | resolve, verify, and read a local source; owns the gates, codec, gittree, archive VFS, bundle, store reader |
| `basilisk-typeshed-fetch` | yes | the only typeshed network code: metadata, download, gates, store writer |
| `basilisk-checker` | **no** | depends on `basilisk-stubs` only; the fetch crate is not in its dependency graph |
| `basilisk-cli`, `basilisk-lsp` | yes | depend on both, and invoke the fetch crate only from an explicit user action |

A CI check asserts that dependency shape. It is what makes "the checker never
downloads" a property of the build rather than a promise in prose.

## [TYPESHEDRT-ACCEPTANCE] Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

Each checkbox is an independent automated test. The pinned specification says type checkers **“SHOULD resolve modules containing type information”** in its listed order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)); the acquisition mechanics below are Basilisk policy where that specification is silent.

### [TYPESHEDRT-ACCEPTANCE-SOURCE] Source acquisition and identity {#TYPESHEDRT-ACCEPTANCE-SOURCE}

Step 3 identifies **“Typeshed stubs for the standard library”**, while the same pinned text does not prescribe transport, cache age, or commit selection ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Never clones.** Instrument process spawns and network calls; assert acquisition issues an HTTPS archive fetch and **never** invokes `git`, `git clone`, or a Git transport.
- [ ] **Checking is offline.** Instrument every socket; run `basilisk check`, the LSP over a workspace, and MCP against a pin, a custom folder, and the bundle, and assert **zero** network calls in every case — including when the pin is missing.
- [ ] **Structural segregation.** Assert `basilisk-stubs` and `basilisk-checker` have no HTTP dependency in their resolved dependency graph, and that `basilisk-typeshed-fetch` is absent from the checker's graph ([§TYPESHEDRT-SEGREGATION](#TYPESHEDRT-SEGREGATION)).
- [x] **One generation:** give `A`, `B`, and the bundled snapshot conflicting sentinels; after activating `B`, assert every name/body/distribution lookup reads `B` and no fallback lookup occurs.
- [ ] **Fails hard:** a pin with no store entry and no SHA match in the bundle, and a custom folder that does not exist, both refuse to analyse, name the missing SHA/path, emit `NO SOURCE`, and never substitute another source or an untyped stdlib.
- [x] **Activation gates:** corrupt shape/tree/license metadata; add duplicate, absolute/`..`, escaping-link, over-count, and zip-bomb entries; assert rejection even with content verification off.
- [x] **Immutable ZIP/VFS:** interrupt acquisition and mutate a cached ZIP; assert neither activates, reuse detects mutation by ZIP SHA-256, and every `.pyi` read comes from the accepted ZIP.
- [x] **Concurrent callers:** CLI/LSP/MCP callers observe one complete identity and one atomic promotion.
- [x] **Cache fingerprint:** different SHAs, custom-tree identities, and bundled identity miss the checker cache; identical identities hit it.

### [TYPESHEDRT-ACCEPTANCE-VERIFY] Integrity, verification, and cache {#TYPESHEDRT-ACCEPTANCE-VERIFY}

Trusted GitHub metadata binds commit to tree; a user pin selects the commit, and
Git-tree verification binds VFS-consumed bytes to that tree
([§STUBRES-TYPESHED-ACQUIRE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE)).

- [x] **Tree binding:** two archive encodings of one tree pass and any content mutation fails; a pin alone proves nothing because Git commits identify trees, not ZIP hashes ([Git `commit-tree`](https://git-scm.com/docs/git-commit-tree)); verified metadata reports only its GitHub/TLS trust boundary, not a signed typeshed release.
- [ ] **Offline pin verification:** the stored commit object hashes to the pinned SHA, its tree SHA matches the re-hashed stored tree, and mutating any stored byte — or the commit object itself — fails the pin with the network unavailable.
- [ ] **No waiver:** assert there is no configuration key, CLI flag, or wire field that disables pin verification.
- [ ] **Store is inert:** the checker never creates, repairs, or evicts a store entry; a deleted entry stays deleted until a download recreates it; entries never expire with age.
- [ ] **Atomic download:** interrupt at metadata, mid-archive, at each gate, and at commit-object reconstruction; assert no partial or unverified store entry and no `typeshed-commit` write survives any of them.
- [ ] **License drift:** change the approved path+SHA-256 manifest for any relevant root/nested `LICENSE*`/`NOTICE*` on the download and store-read paths; block and report `LICENSE CHANGED`.
- [ ] **GitHub only:** there is no mirror setting; the download contacts only `api.github.com` and `codeload.github.com`, and the credential goes nowhere else.
- [ ] **Status routing:** compose `UNPINNED` + `USER-MANAGED SOURCE`, and `NO SOURCE` alone; assert CLI uses stderr, LSP uses `showMessage`/Service Info (not `publishDiagnostics`), and MCP returns the same ordered structured warnings.

### [TYPESHEDRT-ACCEPTANCE-OVERRIDES] Explicit user sources {#TYPESHEDRT-ACCEPTANCE-OVERRIDES}

Pinned step 3 says a supplied custom typeshed **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Exact commit:** configure full SHA `A`; assert exact tree/VFS bytes, later `main` movement has no effect, and unavailable `A` never substitutes another bundled SHA.
- [ ] **Pinned reuse:** a store entry for `A` is reused regardless of age, re-verified by hashing every time, and only deletion ends reuse. The pin never expires or changes.
- [ ] **Download latest:** resolve `main` to `B`, invoke **Download latest**, and assert `B` lands in the store AND `typeshed-commit` is written to `B` in one action; assert the same action offline writes neither.
- [ ] **Download pinned:** with `typeshed-commit = A` absent from the store, invoke **Download pinned** and assert `A` lands in the store and the configuration is untouched.
- [ ] **Not-pinned advisory:** the bundled default and Custom report `UNPINNED`; only explicit `typeshed-commit` suppresses it; status never becomes a Python diagnostic.
- [x] **Custom tree:** conflicting custom/download/bundle data resolves custom verbatim, reports user-managed terms without assuming Apache/MIT, and bypasses every other step-3 lookup.
- [x] **Custom miss:** omit `X` only from custom while putting it in download/bundled; assert resolution goes directly to step 4 and never rescues `X` from another step-3 source.
- [x] **Configuration validation:** cover wrong key types, source conflicts,
  absolute/workspace-relative paths, required `stdlib/`, nonexistent paths,
  malformed trees, and deterministic fail-closed errors.

### [TYPESHEDRT-ACCEPTANCE-TARGET] Python target semantics {#TYPESHEDRT-ACCEPTANCE-TARGET}

The pinned stub specification says checkers should fully support **“Simple version and platform checks”**; its directives say checkers are **“expected to understand simple version and platform checks”** using `sys.version_info` and `sys.platform` ([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst), [directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst), both `python/typing@6ef9f77`).

- [x] **Same SHA, different targets:** run two target versions against one SHA; assert acquisition identity is unchanged while `stdlib/VERSIONS` admits the fixture's target-specific modules.
- [x] **Guard selection:** concrete target selects one version/platform branch; `All` requires validity across alternatives and never exposes a one-platform-only name (#318 regression).
- [x] **Target environment:** cross-version and multi-root checking use each owning root's target interpreter `site-packages`/`dist-packages`, including an explicit Python-binary override; no root may inherit another root's packages.
- [x] **No commit inference:** instrument the network and change only `python-version`/`python-platform`; assert no different SHA is selected, guessed, or fetched.
- [x] **No manufactured target:** assert configuration, generated data, and bundled data contain no Python-version-to-SHA map and no fixed Python target appears without project/interpreter evidence.

### [TYPESHEDRT-ACCEPTANCE-RESOLUTION] Resolution and stub semantics {#TYPESHEDRT-ACCEPTANCE-RESOLUTION}

The pinned specification orders manual stubs, user code, stdlib typeshed, stub packages, inline `py.typed`, and optional vendored third-party stubs; it also says checkers **“MUST maintain the normal resolution order of checking `*.pyi` before `*.py` files”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Six steps:** collide `X` at steps 1–5, remove each winner, then assert step 6's deliberate absence and unresolved; separately prove installed untyped `.py` resolves as untyped.
- [x] **Stub package versus inline:** install `foopkg-stubs` beside inline `py.typed` `foopkg`; assert step 4 wins over step 5.
- [x] **Package misses:** complete stub-package miss stops; `partial\n` and stub-only namespace (no `__init__.pyi`) misses continue to steps 5/6.
- [x] **`.pyi` precedence:** place `.pyi` and `.py` for one module at the winning location; assert only `.pyi` supplies the public interface.
- [x] **#312/#318 exports:** with an exact MicroPython snapshot, `import
  asyncio` exposes `asyncio.sleep`, `asyncio.Task`, and `asyncio.run` through the
  production module binding; redundant aliases, specified `__all__` mutations,
  stars, private exclusion, cycles, and long chains resolve without target unions.

### [TYPESHEDRT-ACCEPTANCE-HOVER] #288 and #289 behavior {#TYPESHEDRT-ACCEPTANCE-HOVER}

Pinned stub and constructor rules govern these tests
([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst),
[constructors](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/constructors.rst),
both `python/typing@6ef9f77`).

- [x] **#289:** real `unittest.mock.Mock` plus fixtures cover special metaclass `__call__`, inherited non-`object` `__new__`/`__init__`, object fallback, binding, overloads/unions, and non-instance termination.
- [x] **#288:** real `str.join` preserves overloads, return types, `LiteralString`, `/`, and receiver specialization/removal in hover and call checking; `.pyi` mutation proves no hand table.
- [x] **Offline parity:** repeat #288/#289 on the **bundled ZIP** (network removed) and assert identical real-body signatures — the offline floor is not names-only.
- [x] **Override behavior:** repeat both with conflicting custom stubs and assert custom signatures/provenance.
- [x] **Shared declaration:** assert hover, signature help, completion, and go-to-definition use the same indexed declaration and source identity.
### [TYPESHEDRT-ACCEPTANCE-GATES] Licensing and release gates {#TYPESHEDRT-ACCEPTANCE-GATES}

Bundling invokes Apache 2.0 §4; runtime downloads do not make Basilisk the
redistributor ([§STUBRES-TYPESHED-LICENSE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-LICENSE)).

- [ ] **Every artifact:** exact bundled-SHA composite LICENSE (including MIT notice), conditional root/nested NOTICE/license files, retained notices, and modified-file marks ship in every binary/package/VSIX.
- [x] **Policy metadata:** `THIRD-PARTY-LICENSES`/`NOTICES` record typeshed, licenses, URL, exact SHA, derived indexes, and repackaging; any license identity/NOTICE change fails for human review.
- [ ] **MCP provenance:** structured status includes active source, full commit/tree identity, license status/reference (custom may say `not supplied`), and ordered warnings; there are no separate transport/provenance fields — the active source is the trust story.
- [x] **Conformance:** run the unmodified `python/typing@main` conformance harness against the clean release binary; require 100% and zero false positives, including no source-status diagnostics.
- [x] **Docs integrity:** validate the six-step Mermaid flow, anchors, links, and the full `6ef9f7719ecfff09dad8724ef42b621fd994fb5e` pin in every touched typeshed section.
- [ ] **No forbidden policy:** reject any network call on the analysis path, any
  automatic download, any fallback to a SHA the user did not name, any
  verification waiver, Python-version-to-SHA maps, fixed Python defaults, and
  `git clone`; preserve exact immutable pins, re-hashed stored bytes, and custom paths.
