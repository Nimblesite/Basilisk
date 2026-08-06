# Stub Resolution & Type Provenance — Specification {#STUBRES-OVERVIEW}

> **Crate**: `basilisk-stubs` (resolution and the offline step-3 sources — the bundled stdlib ZIP, the content-addressed commit store, and offline pin verification; it links no HTTP client, see [§STUBRES-TYPESHED-OFFLINE](#STUBRES-TYPESHED-OFFLINE)), `basilisk-typeshed-fetch` (the segregated, user-invoked download that is the only code which opens a connection, see [§STUBRES-TYPESHED-DOWNLOAD](#STUBRES-TYPESHED-DOWNLOAD)), `basilisk-config` (overrides)
> **Related**: [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY) — `PackageRegistry` accelerates stub discovery

---

## Static Resolution Model {#STUBRES-STATIC-MODEL}

Resolution is a static filesystem search in the pinned six-step order; it never
executes Python or models runtime import hooks
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Computed module names and `sys.meta_path`/custom-loader results are therefore not
guessed. Their typed solution is a `.pyi` at step 1, 4, or 5.

After every step misses, `ImportResolution::Unresolved` binds implicit `Any` and
emits `imports_unresolved`; only an explicit stub `__getattr__(name: str) -> Any`
opts out ([§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL),
[CHKARCH-STRICTNESS-ANY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-ANY)).

---

## Import Resolution Order {#STUBRES-PEP561}

Basilisk adopts the typing specification's upstream **SHOULD** order as an
internal **MUST** —
[Distributing type information → Import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
(the normative successor to [PEP 561](https://peps.python.org/pep-0561/)).
The table maps that upstream order to Basilisk; the linked typing specification
is authoritative for the general rule. Its full normative text is reproduced
verbatim in [§STUBRES-PEP561-NORMATIVE](#STUBRES-PEP561-NORMATIVE) so the mapping
can be checked against the source directly, not paraphrased.

### Normative text, verbatim {#STUBRES-PEP561-NORMATIVE}

Quoted in full (no elision of any step) so Basilisk's behaviour can be audited
against the letter of the specification. Source pinned to
[`python/typing@6ef9f77` · `docs/spec/distributing.rst`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)
(rendered: [Distributing type information → Import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)).
Verify the pin before relying on it: the SHA is a fixed point in history, and
`main` may have advanced.

**Import resolution ordering** — the complete ordered list, verbatim:

> The following is the order in which type checkers supporting this specification
> SHOULD resolve modules containing type information:
>
> 1. Stubs or Python source manually put in the beginning of the path. Type
>    checkers SHOULD provide this to allow the user complete control of which
>    stubs to use, and to patch broken stubs or inline types from packages. In
>    mypy the `$MYPYPATH` environment variable can be used for this.
> 2. User code - the files the type checker is running on.
> 3. Typeshed stubs for the standard library. These will usually be vendored by
>    type checkers, but type checkers SHOULD provide an option for users to
>    provide a path to a directory containing a custom or modified version of
>    typeshed; if this option is provided, type checkers SHOULD use this as the
>    canonical source for standard-library types in this step.
> 4. Stub packages - these packages SHOULD supersede any installed inline
>    package. They can be found in directories named `foopkg-stubs` for package
>    `foopkg`.
> 5. Packages with a `py.typed` marker file - if there is nothing overriding the
>    installed package, *and* it opts into type checking, the types bundled with
>    the package SHOULD be used (be they in `.pyi` type stub files or inline in
>    `.py` files).
> 6. If the type checker chooses to additionally vendor any third-party stubs
>    (from typeshed or elsewhere), these SHOULD come last in the module
>    resolution order.

The two immediately following clauses are also part of this contract, verbatim:

> If typecheckers identify a stub-only namespace package without the desired
> module in step 4, they should continue to step 5/6. Typecheckers should identify
> namespace packages by the absence of `__init__.pyi`. This allows different
> subpackages to independently opt for inline vs stub-only.

> Type checkers that check a different Python version than the version they run
> on MUST find the type information in the `site-packages`/`dist-packages` of that
> Python version. This can be queried e.g. `pythonX.Y -c 'import site;
> print(site.getsitepackages())'`. It is also recommended that the type checker
> allow for the user to point to a particular Python binary, in case it is not in
> the path.

**Stub files, `py.typed`, the `-stubs` naming scheme, partial stubs, and `.pyi`
precedence** — the applicable normative clauses, in source order, from the same
pinned document (linked examples and packaging guidance are not restated):

> Package maintainers who wish to support type checking of their code MUST add a
> marker file named `py.typed` to their package supporting typing. This marker
> applies recursively: if a top-level package includes it, all its sub-packages
> MUST support type checking as well.

> The name of the stub package MUST follow the scheme `foopkg-stubs` for type
> stubs for the package named `foopkg`. […] For stub-only packages adding a
> `py.typed` marker is not needed since the name `*-stubs` is enough to indicate
> it is a source of typing information.

> For the benefit of type checking and code editors, packages can be "partial".
> This means modules not found in the stub package SHOULD be searched for in
> parts five and six of the module resolution order below, namely inline
> packages and any third-party stubs the type checker chooses to vendor. Type
> checkers should merge the stub package and runtime package directories.
>
> This can be thought of as the functional equivalent of copying the stub
> package into the same directory as the corresponding runtime package and type
> checking the combined directory structure. Thus type checkers MUST maintain
> the normal resolution order of checking `*.pyi` before `*.py` files. If a stub
> package distribution is partial it MUST include `partial\n` in a `py.typed`
> file.
>
> For stub-packages distributing within a namespace package, the `py.typed` file
> should be in the submodules of the namespace. Type checkers should treat
> namespace packages within stub-packages as incomplete since multiple
> distributions may populate them. Regular packages within namespace packages
> in stub-package distributions are considered complete unless a `py.typed`
> with `partial\n` is included.

Every row below applies the complete pinned quotation above from
[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst).
Runtime acquisition only chooses the data used at step 3; it does not add or
reorder a resolution step.

### Basilisk mapping {#STUBRES-PEP561-MAPPING}

| Spec step | Basilisk mechanism | Config key |
|---|---|---|
| 1 — manual path head | User `.pyi` in `stub-paths`, generated `.basilisk/stubs/`, and `.pyi` or Python source in manual `extra-paths`; `.pyi` precedes `.py` at each location. These MAY shadow every later step. | `stub-paths`, `extra-paths` |
| 2 — user code | Workspace `.pyi`/`.py` under roots / `include`, with `.pyi` first. | roots, `include` |
| 3 — stdlib typeshed | One selected source, always already on this machine: a custom `typeshed-path`, or the pinned commit — the complete typeshed `stdlib/` tree, third-party `stubs/` excluded ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)). Checking never downloads ([§STUBRES-TYPESHED-OFFLINE](#STUBRES-TYPESHED-OFFLINE)). | `typeshed-path`, `typeshed-commit`, `typeshed-store-path` |
| 4 — stub-only packages | Installed `foopkg-stubs` / typeshed `types-foopkg` distributions, discovered in site-packages. They supersede an inline-typed install of the same package. | (auto) |
| 5 — `py.typed` packages | Installed packages shipping a `py.typed` marker (stubs in `.pyi` or inline in `.py`). | (auto) |
| 6 — vendored third-party stubs | Basilisk vendors none for resolution. The typeshed distribution map drives only the "install stubs" quick fix ([§STUBRES-CODEACTIONS](#STUBRES-CODEACTIONS)). | — |

A complete step-4 package stops on a miss; a partial package or stub-only
namespace miss continues to steps 5–6. Steps 4–5 use the target interpreter's
`site-packages`/`dist-packages`, not the process running Basilisk. This is the
pinned specification's only Python-version-to-storage relationship; it never
selects a typeshed commit.

A module that matches no step resolves to `Unknown` and `imports_unresolved`
fires ([§STUBRES-PROVENANCE-DIAG](#STUBRES-PROVENANCE-DIAG)).

> **uv fast path**: In uv projects, steps 4–5 are accelerated by the `PackageRegistry` parsed from `uv.lock`. The registry knows every installed package and whether a companion stub package exists — no site-packages directory walk needed. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY).

### Custom typeshed override {#STUBRES-CUSTOM-TYPESHED}

The pinned typing specification says a configured custom typeshed should be the
"canonical source for standard-library types in this step" ([step 3,
`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Basilisk exposes that source as `typeshed-path`:

```toml
[tool.basilisk]
typeshed-path = "typeshed-micropython"
```

That directory is the sole step-3 source and excludes the pin and the bundled
ZIP. A missing module continues at step 4; it never falls back to another
typeshed. Relative paths resolve from the workspace root and stdlib stubs live at
`<typeshed-path>/stdlib/<module>.pyi`. `stub-paths` remains the separate step-1
override.

### Resolution flow {#STUBRES-RESOLUTION-FLOW}

The flow is the six-step order quoted verbatim above and pinned to
[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst).
Step 3 receives one preselected source; acquisition details never branch the
import order.

```mermaid
flowchart LR
    A["import X"] --> S1{"1 · manual stub?"}
    S1 -- hit --> R1["UserStub"]
    S1 -- miss --> S2{"2 · user code?"}
    S2 -- hit --> R2["Source"]
    S2 -- miss --> S3{"3 · selected stdlib source?"}
    S3 -- hit --> R3["Typeshed resolved (custom folder / pinned local tree)"]
    S3 -- miss --> S4{"4 · stub package?"}
    S4 -- module hit --> R4["StubPackage"]
    S4 -- none --> S5{"5 · py.typed package?"}
    S4 -- package miss --> P4{"partial or namespace?"}
    P4 -- yes --> S5
    P4 -- no --> U
    S5 -- py.typed hit --> R5["InlineTyped"]
    S5 -- untyped .py hit --> R5U["UntypedImport"]
    S5 -- miss --> S6["6 · vendored third-party stubs: none"]
    S6 --> U["Unknown → imports_unresolved"]
```

---

## Stub Discovery Engine {#STUBRES-ENGINE}

`basilisk-stubs` provides stub resolution.

### Type model {#STUBRES-TYPE-MODEL}

[`models/stub_resolution.td`](../../models/stub_resolution.td) defines
`StubResolution`, `StubSource`, and `StubTier`. Its variants record steps 1,
3–5; Custom typeshed is Tier1 and never reported as CPython.

### Standard-library typeshed source {#STUBRES-TYPESHED}

The pinned typing specification names "Typeshed stubs for the standard library",
says they are "usually" vendored, and makes a configured custom tree canonical
([step 3, `python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
The specification does not prescribe transport, caching, commits, or freshness.
Independent precedent is bundle plus whole-source override: ty uses stubs
“bundled as a zip file in the binary” ([`astral-sh/ruff@035ebc3`](https://github.com/astral-sh/ruff/blob/035ebc332af34b7e301606dcc74d997092be2316/crates/ty_project/src/metadata/options.rs#L842-L844));
mypy uses a custom directory “instead of the typeshed that ships with mypy”
([`python/mypy@d091daa`](https://github.com/python/mypy/blob/d091daa83f95ad0b505cc4f783a5273c0dced5d1/docs/source/config_file.rst#L1067-L1070));
Pyright “ships with a bundled copy of typeshed type stubs”
([`microsoft/pyright@1bec65c`](https://github.com/microsoft/pyright/blob/1bec65c15fba26016281d44d977bf667b89b9d30/docs/configuration.md#L23)).
Basilisk likewise never mixes a source's names, bodies, `VERSIONS`, or indexes.

There are exactly **three** sources, all already on this machine when checking
starts. There is no "track latest" source: freshness is an action a person takes
([§STUBRES-TYPESHED-DOWNLOAD](#STUBRES-TYPESHED-DOWNLOAD)), never something the
checker does on their behalf.

| Source | Selected by | Active data |
|---|---|---|
| Pinned commit *(default)* | `typeshed-commit`; unset selects the bundled commit | the local tree carrying exactly that SHA — the embedded ZIP when the SHA is the bundled one, else that commit's store entry |
| Custom folder | `typeshed-path` | that tree verbatim, user-managed |
| PyPI package *(pinned by wheel SHA-256)* | `typeshed-package` | the stored wheel whose SHA-256 is the pin ([§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI)) |

All three fail closed. Custom is reported unpinned
([§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)); a *module* miss in a custom
tree still continues to step 4.

#### The checker never downloads {#STUBRES-TYPESHED-OFFLINE}

Analysis performs no network activity of any kind — structurally, not by
discipline: the crate the checker links against contains no HTTP client, so the
analysis path cannot reach the network even by mistake. Step 3 is a local
lookup: the source is present and verifies, or it is missing/corrupt and the
checker **fails hard** — it refuses to analyse, names the SHA it needed, and
never substitutes another source or degrades to an untyped stdlib. That failure
is service status (CLI stderr, LSP `showMessage` + Service Info, MCP), never a
Python diagnostic, so it can never create a conformance false positive.

#### A pin is a verification {#STUBRES-TYPESHED-PIN}

A pin does exactly one thing at check time: **proves the local tree is that
commit**, offline, by hashing bytes already on disk.

1. hash the store entry's saved commit object — it MUST equal the pinned SHA;
2. read the tree SHA out of that verified commit object;
3. re-hash the stored tree into Git tree objects — the root MUST equal that SHA.

Verification is **not waivable**: a pin you can switch off is a pin that does
nothing. The bundle cannot be tree-reconstructed (it is a `stdlib/` subset), so
it keeps its build-time proof — embedded ZIP SHA-256 plus license manifest
([§STUBRES-TYPESHED-BASELINE](#STUBRES-TYPESHED-BASELINE)) — and satisfies a pin
naming its commit.

**Trust boundary.** This proves integrity since acquisition and binds commit→tree
cryptographically; it cannot prove offline that the SHA is an official typeshed
commit. That authenticity rests on GitHub/TLS at download time, and typeshed
publishes no signed release ([Git `commit-tree`](https://git-scm.com/docs/git-commit-tree),
[GitHub Git-commit API](https://docs.github.com/en/rest/git/commits)). Whoever can
rewrite the store can rewrite its commit object with it.

#### A PyPI package pin {#STUBRES-TYPESHED-PYPI}

A third source is a PyPI typeshed distribution pinned by its **wheel SHA-256** —
the hash `uv` records in `uv.lock` `wheels[].hash`
([uv lockfile format](https://docs.astral.sh/uv/reference/files/#lockfile-format);
issue #312). The source is the stored wheel archive: Basilisk reads its
`stdlib/` subtree via the archive VFS, so the checked bytes are the pinned bytes.

Acquisition is segregated and user-invoked (`basilisk typeshed download
--package`), like a commit pin
([§STUBRES-TYPESHED-DOWNLOAD](#STUBRES-TYPESHED-DOWNLOAD)); the fetch crate
downloads the wheel from PyPI, verifies its SHA-256 equals the pin, and stores
it as the wheel entry described in
[§STUBRES-TYPESHED-STORE](#STUBRES-TYPESHED-STORE). Check-time verification is
offline ([§STUBRES-TYPESHED-OFFLINE](#STUBRES-TYPESHED-OFFLINE)): re-hash the
stored wheel and assert equality; missing or mismatched fails hard as
`NO SOURCE`.

**Both halves of the pin are validated before use.** The digest must be exactly
64 hex characters, and the distribution name must be a
[PEP 508](https://peps.python.org/pep-0508/#names) name — ASCII letters, digits,
`.`, `_`, or `-`, beginning and ending with a letter or digit. The name is not
merely a label: it becomes a path segment of the PyPI index URL, so restricting
it to that alphabet is what makes it impossible to write a pin whose lookup
resolves to a different resource. A name outside it is rejected where the pin is
parsed **and** at the transport boundary that builds the URL, so no caller can
route around it; a rejected pin fails closed and no request is made.

**Trust boundary.** Proves the stored wheel is the registry-attested artifact;
cannot prove offline the SHA is an *official* typeshed release (PyPI publishes
no signed releases; authenticity rests on PyPI/TLS at download) — same shape as a
commit pin ([§STUBRES-TYPESHED-PIN](#STUBRES-TYPESHED-PIN)). Advisory behaviour
follows [§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN).

**uv auto-detection.** When `typeshed-package` is unset (and neither
`typeshed-commit` nor `typeshed-path` is set), Basilisk reads `uv.lock`
`wheels[].hash` and, if exactly one recognised typeshed-distribution package
(e.g. `micropython-stdlib-stubs`) is pinned, treats that wheel SHA-256 as the
effective `typeshed-package` pin — no key required (issue #312). Ambiguous (more
than one candidate) or absent → no auto-pin; the bundled default stands with
`typeshed_source_unpinned`. This is an effective-resolution override, never a
configured key: nothing writes it to `pyproject.toml`, and an explicit source
always wins. The recognised-package list is curated so a random dependency
never silently replaces the stdlib source.

#### The store {#STUBRES-TYPESHED-STORE}

One immutable directory per acquired source under `typeshed-store-path`, read by
the checker and written only by the download action. Every entry is
**content-addressed**: the directory name is the digest the entry's bytes must
re-hash to, so reading an entry is what verifies it. There are two entry
shapes, one per acquirable source.

A **commit** entry, addressed by its 40-hex Git commit SHA
([§STUBRES-TYPESHED-PIN](#STUBRES-TYPESHED-PIN)):

```
<typeshed-store-path>/<40-hex commit sha>/
  commit-object   # raw Git commit object; hashes to the directory name
  manifest.json   # the commit's full Git tree listing (path, blob SHA, mode)
  stdlib/… LICENSE NOTICE…
```

A **PyPI package** entry, addressed by its 64-hex wheel SHA-256
([§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI)):

```
<typeshed-store-path>/<64-hex wheel sha256>/
  wheel.whl       # the exact wheel; hashes to the directory name
```

The wheel entry holds **no manifest and no extracted tree**. It does not need
one: a commit entry stores an unpacked tree, so it needs `manifest.json` +
`commit-object` to bind those loose files back to the pinned SHA, whereas the
wheel is stored whole and its own SHA-256 already binds every byte the resolver
will read. The `stdlib/` subtree is read out of the archive in memory rather
than unpacked, so there is nothing on disk left for a manifest to attest.

The two namespaces share one root and **cannot collide**: a Git commit SHA-1 is
always 40 hex characters and a wheel SHA-256 always 64, so the digest length
alone determines which shape an entry is. Both readers reject a digest that is
not exactly their own length before it is used as a path component, so a
malformed pin never reaches the filesystem.

No expiry, no reuse policy, no cache-off mode: an entry is an immutable
artifact and bytes do not go stale. Deleting a directory is the only eviction,
and only a download recreates it.

#### Downloading is a separate component {#STUBRES-TYPESHED-DOWNLOAD}

Acquisition lives outside the checker and outside the configuration editor. It
is the only typeshed code that opens a connection, it runs only when a person
invokes it, and nothing on the analysis path can call it.

| Invocation | Does |
|---|---|
| `basilisk typeshed download` / **Download latest** / `DownloadLatest` | resolve `main` → SHA, acquire it, **write that SHA as `typeshed-commit`** |
| `basilisk typeshed download --commit <sha>` / `DownloadPinned` | acquire the SHA the config already names; writes no config |

Download-pinned is how a teammate or a fresh CI machine materialises someone
else's pin; without it a shared pin is unusable on a machine that never
downloaded it.

Basilisk still never clones: resolve official commit → root-tree metadata over
authenticated HTTPS, download that SHA from GitHub codeload, run the gates
below, reconstruct the commit object and assert it hashes to the requested SHA,
then dump the accepted tree into the store. There is no mirror setting — an
air-gapped or firewalled machine uses a custom folder. A download that fails at
any step writes **nothing** — no partial entry, no unverified entry, no config
change. URLs are redacted in logs. Package-pin acquisition downloads a wheel
from PyPI under the same segregation; see
[§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI).

| Gate | Rule |
|---|---|
| Safety | reject absolute/`..` paths, escaping links, duplicate entries, and entry/decompressed-size limits |
| Shape | require one coherent stdlib tree, `VERSIONS`, and license metadata |
| License | path+SHA-256 manifest for relevant root/nested `LICENSE*`/`NOTICE*` must match a build-approved identity; drift blocks activation for review |
| Content | reconstruct Git trees and match the commit's root-tree SHA |

**Credential.** Requests carry `Authorization: Bearer` from `GITHUB_TOKEN` or
`GH_TOKEN` when either is set to a non-blank value — the names GitHub Actions and
the GitHub CLI already export, so no Basilisk-specific setup is needed. Anonymous
callers share GitHub's unauthenticated rate limit, which a shared CI egress IP
exhausts; an authenticated caller gets a much larger per-token budget
([GitHub rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)).
The credential is sent ONLY to `api.github.com` and `codeload.github.com`, matched
on the parsed authority. The token value is never logged, never rendered in
debug output, and never included in an error; only its presence is recorded.

#### Bundled ZIP snapshot {#STUBRES-TYPESHED-BASELINE}

Basilisk ships a release-pinned ZIP containing every `stdlib/` `.pyi`,
`stdlib/VERSIONS`, the composite root `LICENSE`, root `NOTICE` iff present, and
pertinent nested license/notice files. It is a complete offline step-3 source,
not a names-only baseline; it supplies the bodies and snapshot-derived indexes
needed by #289/#288 offline.

The bundle's manifest digests are enforced twice, and never by a `build.rs`:
the forbidden-policy guard ([§TYPESHEDRT-ACCEPTANCE-GATES](../plans/CHECKER-TYPESHED-RUNTIME-PLAN.md#TYPESHEDRT-ACCEPTANCE-GATES),
`crates/basilisk-stubs/tests/typeshed_forbidden_policy_tests.rs`) bans build
scripts on this crate. Instead, `crates/basilisk-stubs/tests/typeshed_baseline_tests.rs`
verifies `stdlib.zip` and the distribution sidecar against
`data/typeshed/manifest.json` **in the test suite `test-rust.sh` runs before any
release build**, so a corrupt, truncated, or stale asset fails CI — no basilisk
binary (CLI, LSP, or any packaged artifact) is produced without a verified
typeshed standard library. The second enforcement is
`verify_bundled_assets()` in `src/typeshed/bundle.rs`, exercised by
`bundled_assets_match_manifest_and_pass_all_gates`: it re-hashes the embedded
constants and runs every gate over the decoded archive. Because
`include_bytes!` data is the same bytes every process start, this is a build
invariant — activation itself only decodes, never re-hashes, keeping cold
start free of per-process verification of immutable inputs.

**Activation must not touch pages nothing asked to read.** The embedded ZIP
lives in the signed binary's `__TEXT,__const`, where the kernel validates each
page's code signature on its FIRST touch — measured at ~1.6 ms to fault in all
of this bundle's ~2.9 MB. So `decode_zip_static`
(`src/typeshed/codec.rs`) derives every entry's data offset from the trailing
central directory alone; reading the ~750 LOCAL headers scattered through the
archive would fault in essentially all of it, because each sits immediately
before its own data. Exactly one LOCAL header is read — the archive's first, at
offset 0 — and the rest are *proven* rather than probed: the entries must tile
the whole pre-directory region with no gaps, each derived data end landing
exactly on the next entry's LOCAL offset and the last exactly on the central
directory. That chain forces `local_name_len + local_extra_len ==
central_name_len` for every entry, which is precisely the condition under which
the derived offset equals the real one. Any archive arranged otherwise fails the
chain and falls through to the authoritative `decode_zip`, so the fast path can
never guess an offset — it only takes the shortcut where the layout proves it
correct.

#### Precomputed builtins class index {#STUBRES-TYPESHED-BUILTINS-INDEX}

Extracting `builtins.pyi` ([§STUBRES-TYPESHED-VERSION](#STUBRES-TYPESHED-VERSION))
is a pure function of the bundled ZIP and the target, and the largest fixed
cost on a cold `check` (~3 ms of every run). So it is precomputed (`cargo run -p
basilisk-stubs --bin gen_builtins_index`), committed as
`crates/basilisk-stubs/data/typeshed/builtins_index.bin`, and embedded
(`src/typeshed/builtins_index.rs`).

**Every target is covered, not just the unpinned intersection.** A project that
pins `python-version` is the common case, and serving only the no-target case
left exactly those projects paying the live parse on every invocation. The
extracted class map is a step function of the target: of the version only
through the `sys.version_info` comparisons the stub itself makes, and of the
platform only through the `sys.platform` literals it itself names
(`guard::platform_guard_literals`, the sole source of platform sensitivity —
`crates/basilisk-stubs/src/pyi_parser/guard.rs`). The artifact therefore
enumerates one variant per (platform class, minor-version interval), which is a
finite and provably complete set:

- **Platform classes** are `All`, one per named literal, and a single `Other`
  for every platform the stub does not name. `Other`'s completeness is not
  assumed — regeneration extracts three probe platforms spread across the string
  ordering and fails with `PlatformFallbackSplit` if they disagree, which is
  what an ordered `sys.platform` comparison would cause.
- **Version intervals** are found by extracting `(3, 0..=40)` at each platform
  class and collapsing consecutive minors that yield the same map. Generating
  well past every version the file names makes the final interval's open-ended
  reading a measured fact rather than an assumption.

Variants share one deduplicated class-blob pool (`builtins_index/codec.rs`), so
covering all of them costs a fraction of their unpooled size — a ratio a test
pins. A decode reads only the pool blobs its selected variant references.

Three guards keep it fast-path'd but never wrong: the checker consults it only
for a `SourceIdentity::Bundled` snapshot (`builtins_class_map`,
`crates/basilisk-checker/src/imports/builtins.rs`) — pins to other commits and
custom trees always extract live, and so does any target the artifact does not
cover (a non-3 major); the header's bundle SHA-256 must match the manifest or
the loader falls back to live extraction; and CI's drift gate
(`embedded_index_matches_regenerated_bytes`) re-extracts with the real parser
and asserts byte equality, so a bundle refresh cannot land unregenerated. Every
fallback is slower, never different: tests pin the decoded map equal to the live
extraction at every generated minor and at each platform class.

#### License and attribution {#STUBRES-TYPESHED-LICENSE}

The reviewed typeshed `LICENSE`
([`python/typeshed@83c2518`](https://github.com/python/typeshed/blob/83c2518a9e6abbda0c44592c3483de459198f887/LICENSE))
is composite: typeshed says the project uses Apache-2.0 and that parts use other
licenses such as MIT. Basilisk MUST NOT call the selected files Apache-only or
MIT-only.

For every bundled artifact, [Apache-2.0 §4](https://www.apache.org/licenses/LICENSE-2.0.html#redistribution)
requires license delivery, pertinent-notice retention, readable `NOTICE`
attribution when present, and marks on modified upstream files; the composite
also carries the MIT copyright and permission notice. Basilisk policy requires
this with exact copies, SHA comparison, human review on drift, and
`THIRD-PARTY-LICENSES`/`NOTICES` records for source URL, SHA, and repackaging.
Basilisk retains legal files in cached ZIPs and attributes derived indexes with
their source SHA. CLI/MCP expose source, SHA, and license reference; the UI adds
View License. These are provenance policy, not extra Apache mandates. A
custom path is user-managed and MUST NOT be assigned typeshed's terms. Direct
runtime downloads are not Basilisk release artifacts.

#### Source reporting {#STUBRES-TYPESHED-WARN}

The pinned typing specification defines resolution order, not transport status
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Basilisk reports `active_source` plus an ordered `warnings[]`; warnings compose.

**These advisories are real, named Basilisk diagnostics** — deliberately
upgraded from the anonymous `key="VALUE"` log lines they used to be. Each
carries a stable, **descriptive, number-free** code (`typeshed_source_*`, named
like a conformance rule — never a `BSK-####`), a plain-English message, and a
canonical `/errors/<code>` documentation page it deep-links with `see:
https://www.basilisk-python.dev/errors/<code>`, exactly like every other
diagnostic the CLI prints ([§WEBSITE-ERROR-PAGES](WEBSITE-ERROR-PAGES-SPEC.md#WEBSITE-ERROR-PAGES)).
The `code`/`message` pair is the single source of truth: the Rust
`TypeshedWarning` (`crates/basilisk-stubs/src/typeshed/warning.rs`), this table,
and the generated `/errors/<code>` page all state the same thing.

| Condition | Code | Default severity | Persistent status message |
|---|---|---|---|
| no explicit `typeshed-commit` (the bundled commit is a build-time pin, not a user pin) | `typeshed_source_unpinned` | `warning` | the typeshed stubs bundled with Basilisk are not pinned to a commit; set `typeshed-commit` to an exact SHA so type checks stay reproducible across machines and CI |
| custom folder (contents can change on disk) | `typeshed_source_unpinned` | `warning` | the custom typeshed folder is not version-pinned, so its contents can change between runs and checks are not reproducible; version or content-address the folder externally |
| custom folder (user supplies license + contents) | `typeshed_source_user_managed` | `warning` | the custom typeshed is user-managed: you supply its license and contents, so typeshed's license terms are not applied to it |
| approved license/NOTICE identity changed | `typeshed_source_license_changed` | `error` | the bundled typeshed's approved LICENSE/NOTICE changed and needs review; update Basilisk before relying on these stubs |
| pinned commit absent from the store, or verification failed | `NO SOURCE` (terminal failure, not an advisory) | — | `NO SOURCE — <sha> is not on this machine; run Download latest or basilisk typeshed download --commit <sha>` — analysis does not run |

**Routing — out of band on every surface, never a Python diagnostic.** The CLI
prints a rustc-style banner (`<severity>[<code>]: <message>` then `= see:
<docs_url>`) to **stderr**, and keeps the structured `active_source`/identity
fields on a separate `debug` telemetry channel — the human banner is not
`key="VALUE"` telemetry. The LSP surfaces them through `window/showMessage`
plus persistent Service Info, never `publishDiagnostics`. MCP returns them as
structured `{code, message, docs_url}` fields. **Conformance invariant:** no
advisory ever enters the stdout JSON / `publishDiagnostics` stream a conformance
run scores, so it can NEVER create a false positive. The default bundled run
emits exactly one advisory — the `typeshed_source_unpinned` reproducibility
notice — on stderr, which the `python/typing` harness never reads; it therefore
cannot affect the pristine fixture result. That historical result is internal
regression evidence, not a current conformance claim
([§CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).

**Severity is configured exactly like any Basilisk rule.** Each advisory carries
the `basilisk` provenance tag, so it resolves severity through the same
`[tool.basilisk.rules]` / `[tool.basilisk.rule-tags]` machinery
([§CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)): a
per-code entry (`"typeshed_source_unpinned" = "error"`) or a whole-tag entry
(`[tool.basilisk.rule-tags]` `basilisk = "…"`) wins outright. With no table
deciding, each code keeps its intrinsic default — advisory conditions render
`warning`, the elevated license change renders `error`. Grading a code
`disabled`/`off` silences it: the only supported way to make an advisory go
away. The resolved severity sets the banner label and whether the advisory
renders at all; it never moves the advisory onto the scored diagnostic stream.
A verified PyPI package pin ([§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI))
suppresses these advisories — the "specifically instructed to accept" path
(issue #312); without a pin they fire as above.

All surfaces show the full SHA when known; the UI also provides a safe View
License action. MCP fields are `active_source`, commit/tree identity,
`license_status`, immutable license reference (or custom `not supplied`), and
ordered `warnings[]` of `{code, message, docs_url}`. The active source already
names the trust story (custom = user-managed, bundled = build-vetted, exact
commit = attested at download and re-proven offline), so there are no separate
transport or provenance fields.

#### Config keys {#STUBRES-TYPESHED-CONFIG}

The only typing-spec-facing setting is the custom canonical path named by pinned
step 3
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst));
the rest govern the pin and where downloads land, which the specification leaves
open. Every one is exposed as a control in the configuration UI
([§LSPCFGED-TYPESHED](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-TYPESHED)).

| Config key | Type | Default | Meaning | Read by |
|---|---|---|---|---|
| `typeshed-commit` | full SHA | unset _(= the bundled commit)_ | The pinned commit, verified offline. | checker |
| `typeshed-path` | `string` | _(unset)_ | The canonical custom step-3 tree; excludes the pin and the bundle. | checker |
| `typeshed-package` | `name@sha256:<hex>` | _(unset)_ | A PyPI typeshed distribution pinned by wheel SHA-256 ([§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI)). Mutually exclusive with `typeshed-commit` and `typeshed-path`. | checker |
| `typeshed-store-path` | path | OS cache | Where downloads are dumped and pins are resolved. | both |

That is the whole *source-selection* surface: four keys. `typeshed-commit`,
`typeshed-path`, and `typeshed-package` are mutually exclusive — exactly one
source may be active. There are no
cache-reuse, expiry, verification-waiver, or mirror settings, and no one-run
flags: nothing is cached, nothing expires, a pin always verifies
([§STUBRES-TYPESHED-PIN](#STUBRES-TYPESHED-PIN)); commit-pin downloads come from
GitHub, package-pin downloads from PyPI
([§STUBRES-TYPESHED-DOWNLOAD](#STUBRES-TYPESHED-DOWNLOAD),
[§STUBRES-TYPESHED-PYPI](#STUBRES-TYPESHED-PYPI)).

Separately, the **severity** of each source-status advisory
([§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)) is graded through the ordinary
rule tables, not these keys. The three codes — `typeshed_source_unpinned`,
`typeshed_source_user_managed`, `typeshed_source_license_changed` — carry the
`basilisk` tag and resolve severity via `[tool.basilisk.rules]` /
`[tool.basilisk.rule-tags]` exactly like any Basilisk rule
([§CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)),
defaulting to `warning` (advisory) or `error` (the license change) and silenced
by grading the code `off`.

#### Target Python version {#STUBRES-TYPESHED-VERSION}

The pinned typing specification expects checkers to "understand simple version
and platform checks"
([`python/typing@6ef9f77`, directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).
Accordingly, a concrete target filters `stdlib/VERSIONS`, selects version/platform
guards, and supplies the target interpreter's site-packages as required above.
The selected snapshot's exact
[`stdlib/VERSIONS`](https://github.com/python/typeshed/blob/83c2518a9e6abbda0c44592c3483de459198f887/stdlib/VERSIONS)
defines inclusive `X.Y-A.B` or open-ended `X.Y-` lifetimes; a submodule without
its own row inherits the closest listed parent module's lifetime. Both the text
and parsed admission index come from the active snapshot identity.
Platform `All` requires validity across alternatives and MUST NOT expose a name
merely because one branch has it. Python version never selects or guesses a
typeshed commit; no fixed Python default or Python→commit map exists.

### .pyi File Parsing {#STUBRES-PYI}

The pinned specification says checkers should parse supported stub constructs
without contradiction and "fully support" typing features, imports, aliases,
and simple version/platform checks
([`python/typing@6ef9f77`, stub files](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
The `.pyi` index therefore retains declarations, overloads, decorators, class
bases, methods, variables, imports, aliases, and guards; bodies are ignored.
For #289, class hover follows the pinned constructor conversion rules: special
metaclass `__call__`; inherited non-`object` `__new__`; inherited `__init__`;
`cls`/`self` binding; overload preservation; union of applicable synthesized
callables; and early termination for special `__call__` or non-instance
`__new__` returns; classes with neither use `object.__new__`/`object.__init__`
([`python/typing@6ef9f77`, constructors](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/constructors.rst)).
For #288, bound `str.join` removes the displayed receiver while using it to
specialize every overload and return type, preserves `LiteralString` and `/`,
and drives call checking from the same declaration—never a hand table.

**A `.pyi` file is Python, and it is parsed as Python.** Stub indexing runs on
the Ruff AST and resolves decorators, bases, and aliases through binding
resolution ([RESOLV-CANONICAL](CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL)),
exactly as `.py` source does. The recognition rules in
[CHKARCH-RECOGNITION](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION) apply
here without relaxation: a stub may alias what it imports
(`from typing import overload as _overload`), re-export under a different name,
or reach a symbol through a module attribute, and a spelling comparison gets
every one of those wrong. Typeshed does all three.

This is the mandate that makes properties such as `StubFunction::is_overload`
meaningful — the flag records that a decorator **resolved** to the
specification's overload declaration, not that its characters matched. As of
2026-08-06 that flag is unpopulated and `basilisk-stubs` does not compile; it is
the first item of
[ASTREBUILD-PHASE-COMPILE](../plans/CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-PHASE-COMPILE).

#### Re-exports {#STUBRES-PYI-REEXPORTS}

The pinned interface rules say imported symbols are **“private by default”**,
that `__all__` **“overrides all other rules above”**, and that the redundant
aliases plus `from Y import *` re-export forms below are public
([`python/typing@6ef9f77`, library interface and import conventions](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)):

> - `import X as X` (a redundant module alias): re-exports `X`.
> - `from Y import X as X` (a redundant symbol alias): re-exports `X`.
> - `from Y import *`: if `Y` defines a module-level `__all__` list, re-exports
>   all names in `__all__`; otherwise, re-exports all public symbols in `Y`'s
>   global scope.

Simple `sys.version_info` / `sys.platform` guards select one concrete target
branch; `All` requires validity in every platform alternative and never exposes a
name from only one branch. This follows the pinned directive that checkers are
expected to understand those checks
([`python/typing@6ef9f77`, directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).

Star targets resolve **inside the re-exporting stub's own source root first**;
a target absent there falls back to the active step-3 Typeshed source
(GitHub #312 follow-up). A user stub may legitimately re-export from a module
owned by a different stub source — MicroPython's `uio.pyi` is just
`from io import *` with `io` in the custom typeshed's `stdlib/` tree — and the
walk keeps recursing through whichever source resolved each target, so a
stdlib package reached this way still follows its own relative re-exports
within the snapshot. Local-first ordering means a sibling stub always shadows
a same-named fallback module; the snapshot is a fallback, never an override.

---

## Type Provenance {#STUBRES-PROVENANCE}

`TypeProvenance` records source annotations, user stubs, built-in/custom typeshed,
community/generated stubs, or untyped imports; there is no `TrackedType` wrapper.

### Diagnostic Behaviour by Provenance {#STUBRES-PROVENANCE-DIAG}

| Resolution/provenance | Import-site diagnostic | Downstream type errors | LSP hover | Code Action |
|------------|-----------|----------------------|-----------|-------------|
| Source | none | normal errors | shows inferred type | — |
| StubUser | none | normal errors | shows stub type | — |
| StubTier1 | none | normal errors | shows stub type + "(typeshed)" | — |
| StubCustomTypeshed | none | normal errors | shows stub type + "(custom typeshed)" | — |
| StubTier2 | none | normal errors | shows type + "(community stub)" | — |
| StubTier3 | none | diagnostics carrying this provenance become info | shows type + "(best-effort stub, may be inaccurate)" | — |
| Unresolved (`Untyped`) | `imports_unresolved` error by default | dependent cascades suppressed | no type information available | add dependency or sync |
| Installed untyped (`Untyped`) | opt-in `BSK-0152`; off by default | normal resolved-source analysis | "(no type stubs available)" | install a published stub package or create a local stub |

### Typing status of installed packages {#STUBRES-TYPING-STATUS}

The two untyped states do not share a diagnostic. A terminal unresolved import
emits `imports_unresolved` once and suppresses dependent cascades. An installed
site-packages `.py` without `py.typed` is resolved, never emits
`imports_unresolved`, and emits `BSK-0152` only when the project opts in.

The rule behind that split: a `py.typed` marker governs **provenance and
completeness classification, never resolution**. Its absence downgrades what
Basilisk claims to know about a module; it does not make the module missing.
Conflating the two would report `imports_unresolved` — "this import cannot be
found" — for a package that is installed and importable, which is simply false,
and would then suppress the dependent cascade and hide real errors behind it.
Enforced at steps 4-5 of the resolution order ([STUBRES-PEP561-MAPPING]).

### Code Actions for Unresolved Imports {#STUBRES-CODEACTIONS}

Diagnostics offer one-click actions, not shell instructions; local `.pyi` files
are pinned step-1 stubs
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

| Diagnostic | Scenario | Code Action | LSP Command |
|------------|----------|-------------|-------------|
| imports_unresolved | Package not installed | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package absent or transitive-only | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package declared but not synced | "Sync environment" | `basilisk.uv.sync` |
| BSK-0152 | Package installed, typeshed stub exists | "Install type stubs: `types-{pkg}`" | `basilisk.uv.addDev` |
| BSK-0152 | Package installed, **no** typeshed stub | "Create local type stub for `{pkg}`" | `basilisk.stubs.createLocal` |

Commands execute the action and re-resolve; uv dependency commands also report
progress. Every BSK-0152 offers create-local. See
[LSPUV-ACTIONS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS).

#### Create Local Stub {#STUBRES-CREATE-LOCAL}

`basilisk.stubs.createLocal` creates, but never overwrites,
`.basilisk/stubs/{module}.pyi`. The strict skeleton declares nothing, documents
the explicit `__getattr__ -> Any` opt-out, rebuilds resolution, and links the
[stub-writing guide](https://typing.python.org/en/latest/guides/writing_stubs.html).

#### Add Member {#STUBRES-ADD-MEMBER}

`basilisk.stubs.addMember` handles `imports_module_attribute`: call sites infer
`Any` parameters; plain access adds `attr: Any`. It writes only an existing
workspace `.pyi`, inserts the import once, re-resolves, and leaves tightening to
the developer ([CHKARCH-DIAG-STUB-MEMBER](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)).

### Provenance in Hover {#STUBRES-PROVENANCE-HOVER}

| Cursor on | Hover display |
|-----------|---------------|
| Untyped import | `fastmcp (no type stubs available)` |
| Tier 3 stub symbol | `FastMCP (best-effort stub, may be inaccurate)` |
| Tier 2 stub symbol | `pandas.read_csv(...) (community stub)` |
| typeshed symbol | `os.path.join (typeshed)` |
| custom-typeshed stdlib symbol | `os.uname (custom typeshed)` |
| Tier 1 stub symbol | `requests.get(...) -> Response` (no annotation — trusted) |

> uv projects enrich import hovers with package version and dependency classification from the `PackageRegistry`; see [LSPUV-HOVER](LSP-UV-INTEGRATION-SPEC.md#LSPUV-HOVER).

---

## Configuration {#STUBRES-CONFIG}

### Suppression ownership {#STUBRES-SUPPRESSION}

Stub/import diagnostics use ordinary severity and inline suppression; there is
no stub-specific grammar. `stub-paths` implements pinned step 1, and the step-3
keys live only in [§STUBRES-TYPESHED-CONFIG](#STUBRES-TYPESHED-CONFIG), preserving
the six-step order quoted from
[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst).
All keys use `[tool.basilisk]`; pyright aliases and folder scoping follow
[ANALYSIS-CONFIG-PRI](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI) and
[CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL).

---

## Auto-Stub Generation {#STUBRES-AUTOGEN}

`basilisk stubs generate <package>` (or `--all`) writes hybrid
runtime-introspection/AST output to `.basilisk/stubs/`; `basilisk stubs status`
reports coverage. Generated files are Tier 3 step-1 stubs, consistent with the
pinned rule that a user-supplied stub is considered first
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

### Generation modes {#STUBRES-AUTOGEN-MODES}

`basilisk stubs generate --mode <mode>` selects the backend. There are exactly
three, and the mode is a **command argument, not configuration** — consistent
with [CHKARCH-CONFIGURATION-ONLY], the config file grades rules and never
selects behaviour.

| Mode | Backend | Needs a Python subprocess | Needs `.py` source |
|---|---|---|---|
| `runtime` | `inspect.signature()` in the target interpreter | yes | no |
| `ast` | parses `.py` source with `basilisk-parser` | no | yes |
| `hybrid` (default) | runtime first, falling back to AST per function | yes | for the fallback |

Accuracy runs highest-to-lowest in that same order for anything whose signature
is only knowable at runtime (decorated, C-accelerated, or dynamically built
callables), which is why `hybrid` is the default: it takes the accurate answer
where one exists and degrades rather than failing. `ast` is the only mode that
never launches a subprocess, and it fails with a diagnostic when the package
ships no importable source. Whichever mode produced it, output is tagged
[`StubTier::Tier3`] so downstream diagnostics report best-effort provenance
([STUBRES-PROVENANCE-DIAG]) and never false confidence.

---
