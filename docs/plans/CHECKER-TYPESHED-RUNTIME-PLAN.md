# Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Pinned typing authority**: [`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This supplies the real standard-library `.pyi` data missing in [#324](https://github.com/Nimblesite/Basilisk/issues/324), so [#289](https://github.com/Nimblesite/Basilisk/issues/289) and [#288](https://github.com/Nimblesite/Basilisk/issues/288) can be fixed without changing the typing specification's resolution order.

## Contract {#TYPESHEDRT-MODEL}

Pinned step 3 says **“Typeshed stubs for the standard library”**, says those stubs are **“usually”** vendored, and says a provided custom path **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). “Usually” does not mandate bundling or any Git policy.

Basilisk therefore activates exactly one step-3 source: `typeshed-path`; otherwise an exact user `typeshed-commit` or `python/typeshed@main` verified for this run/session; otherwise the names-only bundled baseline. Custom or downloaded data wholly bypasses bundled and compiled lookups. There is no refresh TTL, automatic stale-checkout fallback, fixed Python default, or Python-version-to-commit map. An explicit commit is deliberate immutable user selection, not automatic staleness.

## Work {#TYPESHEDRT-WORK}

The pinned order puts standard-library typeshed at step 3, stub packages at step 4, inline `py.typed` packages at step 5, and optional vendored third-party stubs last ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Implement only what is needed to preserve that order:

1. Replace authoritative compile-time name lookup with one selected source that supplies module names, real `stdlib/*.pyi` bodies, `stdlib/VERSIONS`, and the distribution map from one identity. A compiled copy may only accelerate the names-only baseline.
2. `acquire(config) -> Custom | Downloaded | Baseline`: return `Custom` immediately; otherwise validate/fetch an exact pin or fetch and verify `main` into a temporary tree; atomically activate one validated SHA; on unpinned failure return `Baseline`, never an older checkout. A validated tree matching an explicit pin remains eligible.
3. Keep only `typeshed-path`, `typeshed-commit`, and `typeshed-cache-path`. A custom-path miss leaves step 3 and proceeds to step 4.
4. Gate first analysis on acquisition, fingerprint checker caches with source identity, and report only verified download (`pinned` when explicit) or warned names-only baseline.

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

Each checkbox is an independent automated test. The pinned specification says type checkers **“SHOULD resolve modules containing type information”** in its listed order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)); the acquisition mechanics below are Basilisk policy where that specification is silent.

### Source acquisition and identity {#TYPESHEDRT-ACCEPTANCE-SOURCE}

Step 3 requires **“Typeshed stubs for the standard library”**, while the same pinned text does not prescribe transport, cache age, or commit selection ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **Current unpinned `main`:** seed cached SHA `A`, advertise remote SHA `B`, acquire, and assert the reported SHA, `.pyi` path, `stdlib/VERSIONS`, module names, and distribution map all come from `B`.
- [ ] **One generation:** give `A`, `B`, loose baseline, and compiled baseline conflicting sentinels; after activating `B`, assert every name/body/distribution lookup reads `B` and no fallback lookup occurs.
- [ ] **Failed update:** seed `A`, fail remote verification, and assert the names-only baseline and exact warning are selected; `A` is never activated and no `.pyi` body is fabricated.
- [ ] **Invalid checkout:** interrupt before promotion and separately corrupt metadata/tree; assert neither partial tree activates, then assert a successful retry atomically activates one complete SHA.
- [ ] **Concurrent callers:** acquire from CLI- and LSP-shaped callers against one cache; assert one complete source identity, one promotion, and no observation of a temporary tree.
- [ ] **Cache fingerprint:** different SHAs, custom-tree identities, and baseline identities miss the checker cache; identical identities hit it.

### Explicit user sources {#TYPESHEDRT-ACCEPTANCE-OVERRIDES}

Pinned step 3 says a supplied custom typeshed **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] **Exact commit:** configure SHA `A`; assert the active tree is byte-identical to `A`, local mutation fails validation, later movement of `main` changes nothing, and target Python never rewrites the pin.
- [ ] **Pinned reuse:** validate `A`, remove the network, and assert that exact immutable checkout remains eligible and reports `pinned`, never `stale`.
- [ ] **Custom tree:** configure conflicting custom, clone, baseline, and compiled signatures; assert the resolved custom path is used verbatim, custom wins, and acquisition plus all other step-3 lookups are bypassed.
- [ ] **Custom miss:** omit `X` only from custom while putting it in clone/baseline; assert resolution goes directly to step 4 and never rescues `X` from another step-3 source.
- [ ] **Path validation:** cover absolute/workspace-relative paths, required top-level `stdlib/`, nonexistent paths, malformed trees, and deterministic diagnostics.

### Python target semantics {#TYPESHEDRT-ACCEPTANCE-TARGET}

The pinned stub specification says checkers should fully support **“Simple version and platform checks”**; its directives say checkers are **“expected to understand simple version and platform checks”** using `sys.version_info` and `sys.platform` ([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst), [directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst), both `python/typing@6ef9f77`).

- [ ] **Same SHA, different targets:** run two target versions against one SHA; assert acquisition identity is unchanged while `stdlib/VERSIONS` admits the fixture's target-specific modules.
- [ ] **Guard selection:** put incompatible declarations behind simple version/platform guards; assert only the matching branch reaches symbols, hover, completion, and diagnostics—never a union of branches.
- [ ] **No commit inference:** instrument Git and change only `python-version`/`python-platform`; assert no different SHA is selected, guessed, or fetched.
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

- [ ] **#289:** resolve `unittest.mock.Mock` from the active SHA; assert hover reaches the class and selected constructor/`__init__` signature with typeshed provenance, not merely import text.
- [ ] **#288:** resolve `str.join` from `stdlib/builtins.pyi`; assert bound-method hover shows that file's parameters/return type; mutate only the fixture signature and assert hover changes, proving no hand-maintained table won.
- [ ] **Override behavior:** repeat both with conflicting custom stubs and assert custom signatures/provenance; repeat on names-only baseline and assert no signature is invented.
- [ ] **Shared declaration:** assert hover, signature help, completion, and go-to-definition use the same indexed declaration and source identity.

### Release gates {#TYPESHEDRT-ACCEPTANCE-GATES}

The full pinned quotation and retained diagram in [STUBRES-PEP561](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561) remain the local audit surface; the maintained typing specification and conformance suite remain the upstream authority.

- [ ] Run a freshly cloned, unmodified `python/typing@main` conformance harness against the clean release binary; require 100% and zero false positives.
- [ ] Validate Mermaid rendering, anchors, links, and the full `6ef9f7719ecfff09dad8724ef42b621fd994fb5e` pin in every touched typeshed section.
- [ ] Reject documentation containing a refresh TTL, automatic stale-checkout fallback, Python-version-to-SHA map, or fixed Python default; permit the explicit user commit pin and custom path.
- [ ] Require the final documentation patch to delete more prose than it adds while retaining the full import-order quotation and resolution diagram.
