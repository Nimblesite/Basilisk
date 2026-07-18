# Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT-OVERVIEW}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Pinned typing authority**: [`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This supplies the real standard-library `.pyi` bodies missing in [#324](https://github.com/Nimblesite/Basilisk/issues/324), so [#289](https://github.com/Nimblesite/Basilisk/issues/289) and [#288](https://github.com/Nimblesite/Basilisk/issues/288) can be fixed — offline and online alike — without changing the typing specification's resolution order.

## Contract {#TYPESHEDRT-MODEL}

Pinned step 3 says **“Typeshed stubs for the standard library”**, says those stubs are **“usually”** vendored, and says a provided custom path **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). “Usually” mandates neither bundling nor any Git policy; the transport below is Basilisk policy where the specification is silent.

Basilisk activates one complete source: Custom folder; Exact commit (fail closed
unless the bundle has that SHA); or Latest with a loudly warned bundled fallback.
It never clones, mixes sources, reuses old unpinned data, maps Python versions to
commits, or changes an exact identity. Latest
warns `UNPINNED` and **Pin current** makes determinism one action away. Custom
is user-managed.

## Work {#TYPESHEDRT-WORK}

The pinned order puts standard-library typeshed at step 3, stub packages at step 4, inline `py.typed` packages at step 5, and optional vendored third-party stubs last ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Implement only what preserves that order:

1. One identity supplies module names, `VERSIONS`, real `.pyi` bodies, and derived indexes; compiled data only accelerates the matching bundle.
2. Resolve trusted commit→tree metadata, stream a safe archive through shape, approved-license/NOTICE, and Git-tree gates, then cache the immutable ZIP and read it through the same VFS. Only content hashing is disableable.
3. Cache reuse re-hashes immutable ZIP bytes without a refresh TTL. Refresh
   TTLs are deliberately excluded: explicit eviction re-downloads the same
   selected SHA; cache-off downloads, validates, and discards. A custom miss
   proceeds to step 4.
4. Gate analysis, fingerprint caches by source identity, and return active source/full SHA plus composable `UNPINNED`, fallback, `LICENSE CHANGED`, `UNVERIFIED`, and user-managed statuses on CLI/LSP/MCP ([§STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

Each checkbox is an independent automated test. The pinned specification says type checkers **“SHOULD resolve modules containing type information”** in its listed order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)); the acquisition mechanics below are Basilisk policy where that specification is silent.

### Source acquisition and identity {#TYPESHEDRT-ACCEPTANCE-SOURCE}

Step 3 identifies **“Typeshed stubs for the standard library”**, while the same pinned text does not prescribe transport, cache age, or commit selection ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Never clones.** Instrument process spawns and network calls; assert acquisition issues an HTTPS archive fetch and **never** invokes `git`, `git clone`, or a Git transport.
- [x] **Current unpinned `main`:** seed cached SHA `A`, advertise remote SHA `B`, acquire, and assert the reported SHA, `.pyi` path, `stdlib/VERSIONS`, module names, and distribution map all come from `B`.
- [x] **One generation:** give `A`, `B`, and the bundled snapshot conflicting sentinels; after activating `B`, assert every name/body/distribution lookup reads `B` and no fallback lookup occurs.
- [x] **Failure rules:** cached unpinned `A` plus failed Latest selects the real-body bundled ZIP and warnings, never `A`; unavailable exact pin fails closed unless the bundle SHA equals it.
- [x] **Activation gates:** corrupt shape/tree/license metadata; add duplicate, absolute/`..`, escaping-link, over-count, and zip-bomb entries; assert rejection even with content verification off.
- [x] **Immutable ZIP/VFS:** interrupt acquisition and mutate a cached ZIP; assert neither activates, reuse detects mutation by ZIP SHA-256, and every `.pyi` read comes from the accepted ZIP.
- [x] **Concurrent callers:** CLI/LSP/MCP callers observe one complete identity and one atomic promotion.
- [x] **Cache fingerprint:** different SHAs, custom-tree identities, and bundled identity miss the checker cache; identical identities hit it.

### Integrity, verification, and cache {#TYPESHEDRT-ACCEPTANCE-VERIFY}

Trusted GitHub metadata binds commit to tree; a user pin selects the commit, and
Git-tree verification binds VFS-consumed bytes to that tree
([§STUBRES-TYPESHED-ACQUIRE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE)).

- [x] **Tree binding:** two archive encodings of one tree pass and any content mutation fails; a pin alone proves nothing because Git commits identify trees, not ZIP hashes ([Git `commit-tree`](https://git-scm.com/docs/git-commit-tree)); verified metadata reports only its GitHub/TLS trust boundary, not a signed typeshed release.
- [x] **Cache controls:** reuse re-hashes cached bytes with no time-based expiry;
  explicit eviction re-downloads the same pin; cache-off leaves no ZIP;
  verification-on reruns the content gate before reporting verified.
- [x] **Verification waived:** skip only tree hashing; safety, shape, approved-license/NOTICE checks still run; all surfaces report `UNVERIFIED` without implying verified provenance.
- [x] **License drift:** change the approved path+SHA-256 manifest for any relevant root/nested `LICENSE*`/`NOTICE*` on Latest, pin, and mirror paths; block, report `LICENSE CHANGED`, and use bundled only under Latest rules.
- [x] **Mirror:** known SHA downloads through `{sha}` and verifies; Latest without official metadata cannot reuse an earlier SHA and falls back loudly.
- [x] **Status routing:** compose `UNPINNED` + fallback + `UNVERIFIED`; assert CLI uses stderr, LSP uses `showMessage`/Service Info (not `publishDiagnostics`), and MCP returns the same ordered structured warnings.

### Explicit user sources {#TYPESHEDRT-ACCEPTANCE-OVERRIDES}

Pinned step 3 says a supplied custom typeshed **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Exact commit:** configure full SHA `A`; assert exact tree/VFS bytes, later `main` movement has no effect, and unavailable `A` never substitutes another bundled SHA.
- [x] **Pinned reuse:** validate `A`, remove the network, and reuse its immutable
  cached ZIP indefinitely; after explicit eviction, reacquire and revalidate
  `A`. The pin itself never expires or changes.
- [x] **Pin current:** in Latest mode, resolve `main` to `B`, invoke **Pin current**, and assert `typeshed-commit` is written to `B`; repeat offline and assert it writes the *bundled snapshot* SHA.
- [x] **Not-pinned advisory:** fresh Latest, bundled fallback, and Custom all report `UNPINNED`; only explicit `typeshed-commit` suppresses it; status never becomes a Python diagnostic.
- [x] **Custom tree:** conflicting custom/download/bundle data resolves custom verbatim, reports user-managed terms without assuming Apache/MIT, and bypasses every other step-3 lookup.
- [x] **Custom miss:** omit `X` only from custom while putting it in download/bundled; assert resolution goes directly to step 4 and never rescues `X` from another step-3 source.
- [x] **Configuration validation:** cover wrong key types, source conflicts,
  absolute/workspace-relative paths, required `stdlib/`, nonexistent paths,
  malformed trees, and deterministic fail-closed errors.

### Python target semantics {#TYPESHEDRT-ACCEPTANCE-TARGET}

The pinned stub specification says checkers should fully support **“Simple version and platform checks”**; its directives say checkers are **“expected to understand simple version and platform checks”** using `sys.version_info` and `sys.platform` ([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst), [directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst), both `python/typing@6ef9f77`).

- [x] **Same SHA, different targets:** run two target versions against one SHA; assert acquisition identity is unchanged while `stdlib/VERSIONS` admits the fixture's target-specific modules.
- [x] **Guard selection:** concrete target selects one version/platform branch; `All` requires validity across alternatives and never exposes a one-platform-only name (#318 regression).
- [x] **Target environment:** cross-version and multi-root checking use each owning root's target interpreter `site-packages`/`dist-packages`, including an explicit Python-binary override; no root may inherit another root's packages.
- [x] **No commit inference:** instrument the network and change only `python-version`/`python-platform`; assert no different SHA is selected, guessed, or fetched.
- [x] **No manufactured target:** assert configuration, generated data, and bundled data contain no Python-version-to-SHA map and no fixed Python target appears without project/interpreter evidence.

### Resolution and stub semantics {#TYPESHEDRT-ACCEPTANCE-RESOLUTION}

The pinned specification orders manual stubs, user code, stdlib typeshed, stub packages, inline `py.typed`, and optional vendored third-party stubs; it also says checkers **“MUST maintain the normal resolution order of checking `*.pyi` before `*.py` files”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [x] **Six steps:** collide module `X` at every step, remove each winner in turn, and assert `1 → 2 → 3 → 4 → 5 → 6 → unresolved`, matching the retained diagram.
- [x] **Stub package versus inline:** install `foopkg-stubs` beside inline `py.typed` `foopkg`; assert step 4 wins over step 5.
- [x] **Package misses:** complete stub-package miss stops; `partial\n` and stub-only namespace (no `__init__.pyi`) misses continue to steps 5/6.
- [x] **`.pyi` precedence:** place `.pyi` and `.py` for one module at the winning location; assert only `.pyi` supplies the public interface.
- [x] **#312/#318 exports:** with an exact MicroPython snapshot, `import
  asyncio` exposes `asyncio.sleep`, `asyncio.Task`, and `asyncio.run` through the
  production module binding; redundant aliases, specified `__all__` mutations,
  stars, private exclusion, cycles, and long chains resolve without target unions.

### #288 and #289 behavior {#TYPESHEDRT-ACCEPTANCE-HOVER}

Pinned stub and constructor rules govern these tests
([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst),
[constructors](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/constructors.rst),
both `python/typing@6ef9f77`).

- [x] **#289:** real `unittest.mock.Mock` plus fixtures cover special metaclass `__call__`, inherited non-`object` `__new__`/`__init__`, object fallback, binding, overloads/unions, and non-instance termination.
- [x] **#288:** real `str.join` preserves overloads, return types, `LiteralString`, `/`, and receiver specialization/removal in hover and call checking; `.pyi` mutation proves no hand table.
- [x] **Offline parity:** repeat #288/#289 on the **bundled ZIP** (network removed) and assert identical real-body signatures — the offline floor is not names-only.
- [x] **Override behavior:** repeat both with conflicting custom stubs and assert custom signatures/provenance.
- [x] **Shared declaration:** assert hover, signature help, completion, and go-to-definition use the same indexed declaration and source identity.

### Licensing and release gates {#TYPESHEDRT-ACCEPTANCE-GATES}

Bundling invokes Apache 2.0 §4; runtime downloads do not make Basilisk the
redistributor ([§STUBRES-TYPESHED-LICENSE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-LICENSE)).

- [ ] **Every artifact:** exact bundled-SHA composite LICENSE (including MIT notice), conditional root/nested NOTICE/license files, retained notices, and modified-file marks ship in every binary/package/VSIX.
- [x] **Policy metadata:** `THIRD-PARTY-LICENSES`/`NOTICES` record typeshed, licenses, URL, exact SHA, derived indexes, and repackaging; any license identity/NOTICE change fails for human review.
- [x] **MCP provenance:** structured status includes active source, full commit/tree identity, transport, license status/reference (custom may say `not supplied`), and ordered warnings.
- [ ] **Conformance:** run the unmodified `python/typing@main` conformance harness against the clean release binary; require 100% and zero false positives, including no source-status diagnostics.
- [x] **Docs integrity:** validate the six-step Mermaid flow, anchors, links, and the full `6ef9f7719ecfff09dad8724ef42b621fd994fb5e` pin in every touched typeshed section.
- [x] **No forbidden policy:** reject stale unpinned fallback,
  Python-version-to-SHA maps, fixed Python defaults, `git clone`, and indefinite
  downloaded-byte reuse; preserve exact immutable pins and custom paths.
