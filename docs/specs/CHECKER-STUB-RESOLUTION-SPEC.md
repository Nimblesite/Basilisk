# Stub Resolution & Type Provenance — Specification {#STUBRES}

> **Crate**: `basilisk-stubs` (resolution, the downloaded `python/typeshed` archive + on-disk cache, and the bundled full-snapshot ZIP), `basilisk-config` (overrides)
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
precedence** — the remaining normative clauses this spec relies on, verbatim from
the same source (bracketed `[…]` marks where non-normative prose between
sentences is omitted; no normative sentence is cut):

> Package maintainers who wish to support type checking of their code MUST add a
> marker file named `py.typed` to their package supporting typing. This marker
> applies recursively: if a top-level package includes it, all its sub-packages
> MUST support type checking as well.

> The name of the stub package MUST follow the scheme `foopkg-stubs` for type
> stubs for the package named `foopkg`. […] For stub-only packages adding a
> `py.typed` marker is not needed since the name `*-stubs` is enough to indicate
> it is a source of typing information.

> If a stub package distribution is partial it MUST include `partial\n` in a
> `py.typed` file. […] Type checkers should treat namespace packages within
> stub-packages as incomplete since multiple distributions may populate them.
> Regular packages within namespace packages in stub-package distributions are
> considered complete unless a `py.typed` with `partial\n` is included.

> Type checkers MUST maintain the normal resolution order of checking `*.pyi`
> before `*.py` files.

Every row below applies the complete pinned quotation above from
[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst).
Runtime acquisition only chooses the data used at step 3; it does not add or
reorder a resolution step.

### Basilisk mapping {#STUBRES-PEP561-MAPPING}

| Spec step | Basilisk mechanism | Config key |
|---|---|---|
| 1 — manual stubs at head of path | User `.pyi` stubs in `stub-paths` directories, plus the auto-discovered `.basilisk/stubs/` cache ([§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL)). They sit at the head of the path and MAY shadow any later module, stdlib or third-party. | `stub-paths` |
| 2 — user code | Workspace `.py` source under the configured roots / `include`. | roots, `include` |
| 3 — stdlib typeshed | One selected source: a custom `typeshed-path`; otherwise the pinned or latest commit downloaded as an archive; otherwise the bundled full-snapshot ZIP ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)). | `typeshed-path`, `typeshed-commit`, `typeshed-cache-path` |
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

That directory is the sole step-3 source and disables archive download and the
bundled ZIP. A missing module continues at step 4; it never falls back
to another typeshed. Relative paths resolve from the workspace root and stdlib
stubs live at `<typeshed-path>/stdlib/<module>.pyi`. `stub-paths` remains the
separate step-1 override; `typeshed-cache-path` only relocates cached downloads.

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
    S3 -- hit --> R3["Typeshed resolved (custom / archive / bundled ZIP)"]
    S3 -- miss --> S4{"4 · stub package?"}
    S4 -- module hit --> R4["StubPackage"]
    S4 -- none --> S5{"5 · py.typed package?"}
    S4 -- package miss --> P4{"partial or namespace?"}
    P4 -- yes --> S5
    P4 -- no --> U
    S5 -- hit --> R5["InlineTyped"]
    S5 -- miss --> S6["6 · vendored third-party stubs: none"]
    S6 --> U["Unknown → imports_unresolved"]
```

Step 3 selects `typeshed-path`; otherwise the downloaded archive for the explicit
`typeshed-commit` or the latest `main` commit; otherwise the bundled full-snapshot
ZIP. A custom-source miss proceeds to step 4. A downloaded archive replaces the
bundled ZIP wholesale.

---

## Stub Discovery Engine {#STUBRES-ENGINE}

`basilisk-stubs` provides stub resolution.

### Type model {#STUBRES-TYPE-MODEL}

`StubSource` records the pinned resolution step; `StubTier` records trust
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
[`models/stub_resolution.td`](../../models/stub_resolution.td) generates the Rust
ADTs:

```typeDiagram
alias PathBuf = String

type StubResolution {
  module: String
  source: StubSource
  pyi_path: Option<PathBuf>
  tier: StubTier
}

union StubSource {
  UserStub
  StubPackage
  InlineTyped
  Typeshed
  CustomTypeshed
}

union StubTier {
  Tier1
  Tier2
  Tier3
}
```

The variants map to steps 1 (`UserStub`), 3 (`Typeshed`/`CustomTypeshed`), 4
(`StubPackage`), and 5 (`InlineTyped`). Custom typeshed is Tier1 and visibly
labelled so an alternative stdlib is never reported as CPython.

### Standard-library typeshed source {#STUBRES-TYPESHED}

The pinned typing specification names "Typeshed stubs for the standard library",
says they are "usually" vendored, and makes a configured custom tree canonical
([step 3, `python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
The specification does not prescribe transport, caching, commits, or freshness.
For precedent, mypy says all users need its bundled stdlib stubs, while ty
documents a vendored stdlib ZIP and custom-tree option
([mypy](https://mypy-lang.blogspot.com/2021/05/the-upcoming-switch-to-modular-typeshed.html),
[ty](https://docs.astral.sh/ty/reference/configuration/)). Basilisk uses one
complete source; names, `VERSIONS`, `.pyi` bodies, and derived indexes MUST NOT
mix across sources.

| Mode | Active source | Failure rule |
|---|---|---|
| Custom folder | `typeshed-path` verbatim | miss continues to step 4; no other step-3 source |
| Exact commit | selected archive (content-attested unless waived), or bundle only at that SHA | otherwise fail closed |
| Latest (default) | current `python/typeshed@main`, once per run/session | never reuse old unpinned data; warn and use bundled ZIP |

Freshness is the default; determinism is one **Pin current** action away. Every
mode without an explicit commit, including Custom folder and bundled, warns that
the project is unpinned ([§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)).

#### Archive acquisition {#STUBRES-TYPESHED-ACQUIRE}

Basilisk never clones. It resolves official commit → root-tree metadata over
authenticated HTTPS, then downloads that SHA from GitHub codeload or a
`typeshed-url` `{sha}` archive mirror. A mirror cannot resolve Latest; if official
metadata is unavailable and no pin exists, Latest warns and uses the bundled ZIP.
URLs are redacted in logs.

**Security boundary.** A reported SHA alone proves nothing about bytes used by
the checker. Trusted GitHub metadata binds a commit to its tree; reconstructing
Git hashes and reading those same bytes binds analysis to that tree. An explicit
pin selects the commit but never replaces the trusted commit→tree mapping. Pin +
mapping + verification provides reproducible attestation; Latest verifies a
runtime-selected tree but remains unpinned. No typeshed release signature is
validated, so official provenance ultimately trusts GitHub/TLS. Custom and
verification-disabled sources MUST NOT be labelled official.

The archive is streamed once through four activation gates, cached as an
immutable ZIP, and read through an archive VFS:

| Gate | Rule |
|---|---|
| Safety | reject absolute/`..` paths, escaping links, duplicate entries, and entry/decompressed-size limits |
| Shape | require one coherent stdlib tree, parseable `.pyi`, `VERSIONS`, and license metadata |
| License | path+SHA-256 manifest for relevant root/nested `LICENSE*`/`NOTICE*` must match a build-approved identity; drift blocks activation for review |
| Content | reconstruct Git trees and match the trusted root-tree SHA; only this gate may be waived |

First acquisition records the accepted ZIP's SHA-256. Reuse hashes the cached
ZIP, detecting mutation without extraction; `.pyi` is read from that same ZIP.
Cache metadata records whether content verification ran; enabling it later MUST
rerun the content gate before the archive can be reported as verified.
Eviction or `typeshed-cache = false` re-downloads and reruns all gates, then
discards it. Archive bytes may change, but verified tree contents for one commit
do not. A pin never expires; a cache entry may. Disabling content verification
reports `UNVERIFIED` and never disables safety, shape, or license review. A fresh
unpinned download is not hermetic.

#### Bundled ZIP snapshot {#STUBRES-TYPESHED-BASELINE}

Basilisk ships a release-pinned ZIP containing every `stdlib/` `.pyi`,
`stdlib/VERSIONS`, the composite root `LICENSE`, root `NOTICE` iff present, and
pertinent nested license/notice files. It is a complete offline step-3 source,
not a names-only baseline; #289/#288 therefore work offline. A compiled name or
distribution index MAY accelerate this exact ZIP but MUST NOT override it or any
custom/downloaded source
([CHKARCH-TESTING-BENCH-RATCHET](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH-RATCHET)).

#### License and attribution {#STUBRES-TYPESHED-LICENSE}

The reviewed typeshed `LICENSE`
([`python/typeshed@83c2518`](https://github.com/python/typeshed/blob/83c2518a9e6abbda0c44592c3483de459198f887/LICENSE))
is composite: typeshed says the project uses Apache-2.0 and that parts use other
licenses such as MIT. Basilisk MUST NOT call the selected files Apache-only or
MIT-only.

For every bundled artifact, [Apache-2.0 §4](https://www.apache.org/licenses/LICENSE-2.0.html#redistribution)
requires license delivery, pertinent-notice retention, readable `NOTICE`
attribution when present, and marks on modified upstream files; the composite
also carries the MIT copyright and permission notice. Basilisk policy implements
this with exact copies, SHA comparison, human review on drift, and
`THIRD-PARTY-LICENSES`/`NOTICES` records for source URL, SHA, and repackaging.
Basilisk additionally preserves license metadata in runtime caches, attributes
derived indexes with their source SHA, and exposes source/SHA/View License in
CLI, UI, and MCP. These are provenance policy, not extra Apache mandates. A
custom path is user-managed and MUST NOT be assigned typeshed's terms. Direct
runtime downloads are not Basilisk release artifacts.

#### Source reporting {#STUBRES-TYPESHED-WARN}

The pinned typing specification defines resolution order, not transport status
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Basilisk reports `active_source` plus an ordered `warnings[]`; warnings compose.

| Condition | Persistent status |
|---|---|
| Latest or bundled without explicit commit | `UNPINNED — Pin current to make this reproducible` |
| Custom folder | `UNPINNED — folder contents can change; use Exact commit for reproducibility` |
| Latest could not resolve, download, or validate | `DOWNLOAD FAILED — using bundled <sha>; may be behind upstream` |
| approved license/NOTICE identity changed | `LICENSE CHANGED — Basilisk update/review required` |
| content verification disabled | `UNVERIFIED — contents were not checked against the selected tree` |
| custom path | `USER-MANAGED SOURCE — license and contents supplied by user` |

CLI uses a stderr status banner without contaminating machine diagnostics; LSP
uses `window/showMessage` plus persistent Service Info, never
`publishDiagnostics`; MCP returns structured status. These warnings therefore
cannot create conformance false positives. All surfaces show the full SHA when
known and a safe View License action. MCP fields are `active_source`, commit/tree
identity, transport, `license_status`, immutable license reference (or custom
`not supplied`), and ordered `warnings[]`. There is no stale state.

#### Config keys {#STUBRES-TYPESHED-CONFIG}

The only typing-spec-facing setting is the custom canonical path named by pinned
step 3
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst));
the rest govern download, caching, and verification, which the specification
leaves open. Every one is exposed as a control in the configuration UI
([§LSPCFGED-TYPESHED](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-TYPESHED)).

| Config key / flag | Type | Default | Meaning |
|---|---|---|---|
| `typeshed-commit` | full SHA | unset | Exact commit; unset selects Latest. |
| `typeshed-url` | URL template | GitHub codeload | Archive mirror containing `{sha}`; does not resolve Latest. |
| `typeshed-cache-path` | path | OS cache | Cached gate-accepted ZIPs. |
| `typeshed-cache` | bool | `true` | Reuse gate-accepted ZIPs; false downloads, validates, and discards. |
| `typeshed-path` | `string` | _(unset)_ | Supply the canonical custom step-3 tree; disables download and the bundled ZIP. |
| `typeshed-verify` | bool | `true` | Content attestation; false reports `UNVERIFIED`. |
| `--no-typeshed-cache` | flag | off | One-run `typeshed-cache = false`. |
| `--no-typeshed-verification` | flag | off | One-run `typeshed-verify = false`; never bypasses other gates. |

#### Target Python version {#STUBRES-TYPESHED-VERSION}

The pinned typing specification expects checkers to "understand simple version
and platform checks"
([`python/typing@6ef9f77`, directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).
Accordingly, a concrete target filters `stdlib/VERSIONS`, selects version/platform
guards, and supplies the target interpreter's site-packages as required above.
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

#### Re-exports {#STUBRES-PYI-REEXPORTS}

A stub's public interface includes the re-exports required by the pinned typing
specification's import conventions
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)):

- **Redundant aliases** — `from y import x as x` and `import x as x` re-export
  `x`.
- **`__all__`** — assignment from a list/tuple plus `+=`, `extend`, `append`, and
  `remove`, including references to a submodule's `__all__`.
- **Star imports** — `from .sub import *` re-exports the target stub's export
  set: its `__all__` when it defines one (authoritative, exactly like runtime
  `import *`), otherwise its public (non-underscore) top-level names and
  re-exports, followed recursively through relative or absolute imports and
  import cycles.

Simple `sys.version_info` / `sys.platform` guards select one concrete target
branch; `All` requires validity in every platform alternative and never exposes a
name from only one branch. This follows the pinned directive that checkers are
expected to understand those checks
([`python/typing@6ef9f77`, directives](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).

---

## Type Provenance {#STUBRES-PROVENANCE}

`TypeProvenance` records source annotations, built-in/custom typeshed,
community/generated stubs, or untyped imports; there is no `TrackedType` wrapper.

### Diagnostic Behaviour by Provenance {#STUBRES-PROVENANCE-DIAG}

| Provenance | imports_unresolved | Downstream type errors | LSP hover | Code Action |
|------------|-----------|----------------------|-----------|-------------|
| Source | not fired | normal errors | shows inferred type | — |
| StubTier1 | not fired | normal errors | shows stub type + "(typeshed)" | — |
| StubCustomTypeshed | not fired | normal errors | shows stub type + "(custom typeshed)" | — |
| StubTier2 | not fired | normal errors | shows type + "(community stub)" | — |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort stub, may be inaccurate)" | — |
| Untyped | error (default) | **suppressed** | shows type + "(no type stubs available)" | one-click install (typeshed) or create-local stub via LSP |

For `Untyped`, `imports_unresolved` fires once, the symbol becomes
`Unknown(Untyped)`, downstream cascades are suppressed, and the LSP offers the
appropriate one-click action.

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

`workspace/executeCommand` reports progress and re-resolves; every BSK-0152 also
offers create-local. See [LSPUV-ACTIONS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS).

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

---
