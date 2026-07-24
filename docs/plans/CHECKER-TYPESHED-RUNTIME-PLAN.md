# Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT-OVERVIEW}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Pinned typing authority**: [`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This supplies the real standard-library `.pyi` bodies missing in [#324](https://github.com/Nimblesite/Basilisk/issues/324), so [#289](https://github.com/Nimblesite/Basilisk/issues/289) and [#288](https://github.com/Nimblesite/Basilisk/issues/288) can be fixed — offline and online alike — without changing the typing specification's resolution order.

**Seventeen acceptance items remain open**, all in
[§TYPESHEDRT-ACCEPTANCE](#TYPESHEDRT-ACCEPTANCE): two on source acquisition and
identity, seven on offline verification and the store, four on explicit user
sources, and four on licensing and release gates. Each is an independent
automated test that does not exist yet; the surrounding prose is settled contract
retained because the implementation cites its anchor.

## Contract {#TYPESHEDRT-MODEL}

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

## Work {#TYPESHEDRT-WORK}

The pinned order puts standard-library typeshed at step 3, stub packages at step 4, inline `py.typed` packages at step 5, and optional vendored third-party stubs last ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Only what preserves that order is in scope:

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

## Segregation {#TYPESHEDRT-SEGREGATION}

| Crate | May link an HTTP client | Role |
|---|---|---|
| `basilisk-stubs` | **no** | resolve, verify, and read a local source; owns the gates, codec, gittree, archive VFS, bundle, store reader |
| `basilisk-typeshed-fetch` | yes | the only typeshed network code: metadata, download, gates, store writer |
| `basilisk-checker` | **no** | depends on `basilisk-stubs` only; the fetch crate is not in its dependency graph |
| `basilisk-cli`, `basilisk-lsp` | yes | depend on both, and invoke the fetch crate only from an explicit user action |

`scripts/check-dependency-shape.sh` asserts that shape from the resolved `cargo
tree` graphs and runs inside `make lint`. It is what makes "the checker never
downloads" a property of the build rather than a promise in prose.

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

Every remaining checkbox is an independent automated test. The pinned specification says type checkers **“SHOULD resolve modules containing type information”** in its listed order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)); the acquisition mechanics below are Basilisk policy where that specification is silent.

### Source acquisition and identity {#TYPESHEDRT-ACCEPTANCE-SOURCE}

Step 3 identifies **“Typeshed stubs for the standard library”**, while the same pinned text does not prescribe transport, cache age, or commit selection ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

Acquisition never invokes `git` or a Git transport; one activated generation
answers every lookup with no fallback; the activation gates reject malformed
shape/tree/license metadata and hostile archive entries even with content
verification off; a cached ZIP that was mutated never activates; concurrent
CLI/LSP/MCP callers observe one atomic promotion; and the checker cache
fingerprint keys on source identity. A pin with no store entry and no bundle SHA
match, and a custom folder that does not exist, both refuse to analyse, name the
missing SHA or path, emit `NO SOURCE`, and substitute nothing.

- [ ] **Checking is offline.** Instrument every socket; run `basilisk check`, the LSP over a workspace, and MCP against a pin, a custom folder, and the bundle, and assert **zero** network calls in every case — including when the pin is missing. The structural half of this is already enforced by `scripts/check-dependency-shape.sh`; what is missing is the end-to-end socket witness over the surfaces that *do* link the fetch crate.

### Explicit user sources {#TYPESHEDRT-ACCEPTANCE-OVERRIDES}

Pinned step 3 says a supplied custom typeshed **“SHOULD [be used] as the canonical source for standard-library types in this step”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

A full SHA selects exact tree/VFS bytes and later `main` movement has no effect.
A store entry for that commit is reused regardless of age, re-verified by hashing
every time, and only deletion ends reuse — the pin never expires or changes.
**Download latest** lands the resolved commit in the store and pegs it as
`typeshed-commit` in one action, writing neither when it fails; **Download
pinned** materialises an existing pin and leaves the configuration untouched. The
bundled default and Custom report `UNPINNED`, and a custom tree resolves verbatim
under user-managed terms, bypassing every other step-3 lookup — a custom miss
goes straight to step 4.

### Python target semantics {#TYPESHEDRT-ACCEPTANCE-TARGET}

The pinned stub specification says checkers should fully support **“Simple version and platform checks”**; its directives say checkers are **“expected to understand simple version and platform checks”** using `sys.version_info` and `sys.platform` ([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst), [directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst), both `python/typing@6ef9f77`).

The Python target filters `stdlib/VERSIONS` and version/platform guards; it never
selects, guesses, or fetches a commit. A concrete target picks one branch, `All`
requires validity across alternatives, each root uses its own interpreter's
packages, and no fixed Python target appears without project or interpreter
evidence.

### Resolution and stub semantics {#TYPESHEDRT-ACCEPTANCE-RESOLUTION}

The pinned specification orders manual stubs, user code, stdlib typeshed, stub packages, inline `py.typed`, and optional vendored third-party stubs; it also says checkers **“MUST maintain the normal resolution order of checking `*.pyi` before `*.py` files”** ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

All six steps are honoured in order, step 4 beats step 5, partial and stub-only
namespace misses fall through, `.pyi` supplies the public interface wherever a
`.py` sits beside it, and re-export chains resolve without target unions.

### #288 and #289 behavior {#TYPESHEDRT-ACCEPTANCE-HOVER}

Pinned stub and constructor rules govern these tests
([distributing](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst),
[constructors](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/constructors.rst),
both `python/typing@6ef9f77`).

Constructor and method signatures come from the real `.pyi` bodies — never a hand
table — on the bundled ZIP as well as online, custom stubs win where supplied,
and hover, signature help, completion, and go-to-definition all read the same
indexed declaration and source identity.

### Licensing and release gates {#TYPESHEDRT-ACCEPTANCE-GATES}

Bundling invokes Apache 2.0 §4; runtime downloads do not make Basilisk the
redistributor ([§STUBRES-TYPESHED-LICENSE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-LICENSE)).

`THIRD-PARTY-LICENSES`/`NOTICES` record typeshed, its licenses, URL, exact SHA,
derived indexes, and repackaging, and any license-identity or NOTICE change fails
for human review; MCP status carries active source, full commit/tree identity,
license status/reference, and ordered warnings with no separate provenance field;
the forbidden-policy guard rejects analysis-path network calls, automatic
downloads, unnamed-SHA fallbacks, verification waivers, Python-version-to-SHA
maps, fixed Python defaults, and `git clone`; the unmodified `python/typing@main`
harness passes at 100% with zero false positives against the clean release
binary; and the documentation integrity gate validates the six-step flow,
anchors, links, and the pin.

- [ ] **Every artifact:** exact bundled-SHA composite LICENSE (including MIT notice), conditional root/nested NOTICE/license files, retained notices, and modified-file marks ship in every binary/package/VSIX. `scripts/verify_release_attribution.py` verifies the binary archives (`--kind binary`) and the wheels (`--kind wheel`) byte-exactly; the VSIX is still only name-presence-checked by `unzip -l`, so it needs the same exact-content verification.
