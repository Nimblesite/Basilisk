# PyPI typeshed package pin — Implementation Plan {#TYPESHEDPYPI-OVERVIEW}

> **Normative spec**: [STUBRES-TYPESHED-PYPI](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PYPI)
> **Issue**: [#312](https://github.com/Nimblesite/Basilisk/issues/312) — the agreed solution:
> pin a PyPI package, verify by SHA, integrate with uv, and suppress the source-status
> advisory unless unpinned.

## Status

| Slice | State |
| --- | --- |
| S1 — Source model & config | ✅ Done |
| S2 — Offline verification backend | ✅ Done |
| S3 — Segregated acquisition | ✅ Done |
| S4 — uv auto-detection | ✅ Done |
| Cross-cutting gates | ✅ Done for this plan (`make test`, clippy, fmt, dep-shape, deslop, conformance all green) |

The one gate this plan does **not** clear is `make bench`, and it is deliberately not this
plan's to clear: the cold-start regression is not caused by S1–S4 and is tracked as its own
task ([Cross-cutting gates](#cross-cutting-gates) records what has actually been established
about it). No benchmark number is claimed here, and the committed baseline must not be
advanced to slower numbers to close it.

## Contract {#TYPESHEDPYPI-CONTRACT}

The spec is the authority; this plan is the means. The agreed behaviour, point for point:

1. **Pin a PyPI package as configuration** — `typeshed-package = "name@sha256:<hex>"`.
2. **Integrate with uv** — the pin's SHA-256 is `uv.lock`'s `wheels[].hash`
   ([verified](https://docs.astral.sh/uv/reference/files/#lockfile-format)); auto-resolve from there.
3. **Verify by SHA** — the source is the *stored wheel* (ZIP via the archive VFS, like the
   bundle), so checked bytes are the pinned bytes; offline re-hash == pin, else `NO SOURCE`.
4. **Default warns, configurable off** — already shipped (`STUBRES-TYPESHED-WARN`/`CONFIG`).
5. **Suppress on pin** — a verified pin emits no advisory ("specifically instructed to accept").

The `basilisk-uv` lockfile parser currently drops `wheels[].hash` into `extra`; S4 captures it.
Store paths do not collide: 40-hex = commit, 64-hex = wheel
([§STUBRES-TYPESHED-STORE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-STORE)).
Both halves of a pin are validated before use: 64-hex digest, and a
[PEP 508](https://peps.python.org/pep-0508/#names) distribution name — the name becomes a path
segment of the index URL, so the alphabet is what stops a pin resolving elsewhere.

## Slices {#TYPESHEDPYPI-SLICES}

### S1 — Source model & config (no transport) · `TYPESHEDPYPI-S1`
`SourceSelection/SourceKind/SourceIdentity::PyPIPackage { name, sha256 }`; selector arm
validates identity + active-source and emits **no** advisory; `SourceBackend::load_pypi_package`
+ `SelectionError/BackendError::PyPIPackage` (fail-closed — store reader lands in S2);
`typeshed-package` parsing (TOML + JSON) + mutual exclusion with `typeshed-commit`/`typeshed-path`;
`TypeshedActiveSource::PyPIPackage`.
**Acceptance:** pin → no advisory; config parses a valid spec, rejects conflicts and malformed
specs; `uri_component()` is opaque.

### S2 — Offline verification backend · `TYPESHEDPYPI-S2`
Store layout `<store>/<64-hex sha256>/wheel.whl` — the wheel alone, no manifest and no unpacked
tree: the wheel's own SHA-256 binds every byte the resolver reads, and the `stdlib/` subtree is
read out of the archive in memory, so there are no loose files for a manifest to attest (unlike a
commit entry, whose unpacked tree needs `manifest.json` + `commit-object` to bind it back to the
pinned SHA). `RuntimeBackend::load_pypi_package`
reads the stored wheel, SHA-256-hashes it, asserts == pin, builds the snapshot from the wheel's
`stdlib/` via the archive VFS, identity `PyPIPackage`, no advisories. Missing → `BackendError::Missing`;
hash mismatch → `BackendError::Corrupt`; both surface as `NO SOURCE`.
**Acceptance:** verified wheel activates & suppresses; missing → `NO SOURCE` naming
`basilisk typeshed download --package`; tampered → `NO SOURCE`.
**Layout (was an open item, now closed):** the Shape gate *is* the guard — it admits a wheel only
if it ships a coherent `stdlib/` tree plus a root `LICENSE`, at download time and again at read
time. No layout *mapping* is assumed: a distribution shipping a different tree is rejected rather
than guessed at, and adding a mapping for one would be a separate, evidence-led change.

### S3 — Segregated acquisition · `TYPESHEDPYPI-S3`
Extend `basilisk-typeshed-fetch` (the only HTTP crate): reject a `<name>` outside the PEP 508
alphabet before a URL exists → resolve `<name>` → PyPI JSON API → select the wheel whose
SHA-256 == pin → re-hash the fetched bytes (the index digest is never trusted) → run the
**Safety and Shape** gates reused from the commit download path → write
`<store>/<sha256>/wheel.whl`. The **License** and **Content** gates are intentionally
skipped: the License gate attests the build-approved *typeshed* `LICENSE` identity (a third-party
wheel ships its own license), and the Content gate reconstructs a Git root tree (a wheel is not a
Git tree) — the wheel SHA-256 *is* the content attestation for a `PyPI` source. Write nothing on
failure. CLI `basilisk typeshed download --package <name>@sha256:<hex>`.
**Acceptance:** verify+store; write-nothing-on-failure; a name outside the PEP 508 alphabet is
refused before a request is built; `scripts/check-dependency-shape.sh` still passes (only
`basilisk-typeshed-fetch` links HTTP); `GITHUB_TOKEN` never sent to PyPI.

### S4 — uv auto-detection · `TYPESHEDPYPI-S4`
Extend `basilisk-uv`'s lockfile parser to capture `wheels[].hash`. When `typeshed-package` is
unset, if `uv.lock` pins exactly one recognised typeshed-distribution package, auto-resolve
`typeshed-package` from its wheel hash. Ambiguous or absent → no auto-pin (bundled default +
`typeshed_source_unpinned`).
**Acceptance:** `uv.lock` pinning the package → auto-pinned, no advisory; two candidates → no auto-pin.

## Out of scope {#TYPESHEDPYPI-OUT}
Signed releases (PyPI has none); sdists (wheel required); pip `requirements.txt`; reading the
installed `site-packages` tree (the stored wheel is the source).

## CI gate {#TYPESHEDPYPI-CI}
`make test` (fail-fast, coverage ratchet up), clippy + fmt at strictest, `make lint` (incl.
`scripts/check-dependency-shape.sh` — `basilisk-stubs` still links no HTTP client), `deslop`, and
and the conformance run recorded unchanged (advisories are environment status, not Python
diagnostics, so they never enter the diagnostic stream) — all green.

`make bench` also ran against the branch, but its outstanding regression is
**not attributable to this plan** and is tracked as its own task; see
[Cross-cutting gates](#cross-cutting-gates) for the evidence. This plan neither claims a benchmark
result nor licenses re-baselining to slower numbers.

## TODO {#TYPESHEDPYPI-TODO}

### S1 — Source model & config (no transport) · `TYPESHEDPYPI-S1`

- [x] `SourceSelection`/`SourceKind`/`SourceIdentity::PyPIPackage { name, sha256 }`.
- [x] Selector arm: validate identity + active-source, emit **no** advisory.
- [x] `SourceBackend::load_pypi_package` + `SelectionError`/`BackendError::PyPIPackage` (fail-closed).
- [x] `typeshed-package` parsing (TOML + JSON) + mutual exclusion with `typeshed-commit`/`typeshed-path`.
- [x] Both halves of the pin validated in the one parser: 64-hex digest, and `is_valid_distribution_name`
      enforcing the [PEP 508](https://peps.python.org/pep-0508/#names) name alphabet (`parse.rs`) — the
      analogue of `is_full_commit_sha` for the package pin.
- [x] `TypeshedActiveSource::PyPIPackage` + `TypeshedSource::PyPIPackage`.
- [x] Config-editor allowlists wired (`TypeshedConfigKey`, `TypeshedSettingKey`, mutation validation, `typeshed_policy_changed`, snapshot projection).
- [x] The package source is **reachable** in the configuration editor. It is the only source with no
      value the editor can supply on the user's behalf — a commit falls back to the bundled SHA and a
      folder comes from the picker — and the server describes sources by their *value*, so until a pin
      exists the snapshot cannot report `PyPIPackage`. Selecting it therefore reveals an empty pin field
      from client presentation state (`pendingPackageEntry`, the same class of state as `advancedOpen`);
      without that, the field that creates a pin would only exist once a pin already existed.
- [x] Selecting the package source is **non-destructive**: it writes nothing. Exclusivity is enforced by
      the write that SETS a source clearing the other two keys in one atomic mutation, never by a
      speculative pre-clear — the same reasoning the cancellable folder picker already used.
- [x] The webview's inline pin pattern matches the server's grammar (PEP 508 name + 64-hex), so a bad
      name is explained in the field instead of round-tripping to a server rejection.
- [x] Tests: selector pin suppresses advisories; config merge/spec-shape; mutation 3-way exclusion;
      the editor rejection carries the parser's specific reason (shape / digest / name), not one generic
      sentence; a real-webview DOM journey selects the package source, is refused in place on a
      non-PEP 508 name, and writes the valid pin with both competing keys cleared in one mutation.

### S2 — Offline verification backend · `TYPESHEDPYPI-S2`

- [x] Store layout `<store>/<64-hex sha256>/wheel.whl` — wheel only, no manifest (the wheel SHA-256 binds
      every byte; nothing is unpacked to disk). Disjoint from commit entries by digest length
      (40-hex = commit, 64-hex = wheel), and both readers reject an off-length digest before it is used
      as a path component. Documented normatively in
      [§STUBRES-TYPESHED-STORE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-STORE).
- [x] `RuntimeBackend::load_pypi_package`: read stored wheel, SHA-256-hash it, assert == pin.
- [x] The digest is validated as canonical 64-hex **before** it is joined onto a path, on the read side as
      well as the write side. Validating afterwards still refuses the snapshot, but only after a
      caller-supplied component had already been used to stat and read a file that may sit outside the
      store; a traversal digest now fails outright and touches nothing.
- [x] Build snapshot from the wheel's `stdlib/` via the archive VFS; identity `PyPIPackage`; no advisories.
- [x] Missing → `BackendError::Missing`; hash mismatch → `BackendError::Corrupt`; both surface as `NO SOURCE`.
- [x] **Open item (resolved for the contract):** the Shape gate IS the guard — a synthetic wheel shipping `stdlib/` + root `LICENSE` passes it (`fake_wheel()` in `testing.rs`, exercised end-to-end by the S3 round-trip test). A real wheel whose layout differs would be rejected at download time and at read time; a layout *mapping* is **not** assumed — if the target package ships a different tree, that is an integration-time follow-up, not an S2 gap.
- [x] Tests: verified wheel activates & suppresses; missing → `NO SOURCE` naming `basilisk typeshed download --package`; tampered → `NO SOURCE` (`wheel.rs`, `runtime/tests.rs`, `selector/tests.rs`).

### S3 — Segregated acquisition · `TYPESHEDPYPI-S3`

- [x] Extend `basilisk-typeshed-fetch` (the only HTTP crate): `<name>` → PyPI JSON API → select wheel whose SHA-256 == pin (`pypi.rs` — `PypiApi` trait + `PypiClient`, anonymous HTTPS-only, no credential).
- [x] Re-hash fetched bytes; run Safety + Shape gates (License/Content intentionally skipped — see slice text); write `<store>/<sha256>/wheel.whl` (`download_package` in `lib.rs`, `write_wheel` in `wheel.rs` — the wheel is the whole entry).
- [x] Write nothing on failure; `GITHUB_TOKEN` never sent to PyPI (`PypiClient` holds no credential).
- [x] `PypiClient::fetch_wheel` re-checks the PEP 508 name alphabet before building the index URL, using
      the same `basilisk_config::is_valid_distribution_name` the parser uses — one alphabet, enforced at
      the boundary that constructs the path segment, so the public `PypiApi` surface cannot be routed
      around by a caller in another crate.
- [x] CLI `basilisk typeshed download --package <name>@sha256:<hex>` (`typeshed_cli.rs` — `--package` flag, mutually exclusive with `--commit`).
- [x] Tests: verify+store round-trip via `read_snapshot`; byte-mismatch/resolve/download failures write nothing; unknown digest fails closed; `--package` CLI exit codes (0/2/3); a traversal/separator/query/escape name is refused before a request exists (`fetch_wheel_refuses_a_name_outside_the_pep_508_alphabet`, hermetic precisely *because* the rejection precedes the transport); `scripts/check-dependency-shape.sh` still passes (only `basilisk-typeshed-fetch` links HTTP).

### S4 — uv auto-detection · `TYPESHEDPYPI-S4`

- [x] Extend `basilisk-uv` lockfile parser to capture `wheels[].hash` (`LockWheel { url, hash, extra }` on `LockPackage`; `extra` no longer swallows it).
- [x] When `typeshed-package` is unset, if `uv.lock` pins exactly one recognised typeshed-distribution package, auto-resolve from its wheel hash (`find_typeshed_package_pin` + `resolve_typeshed_package_pin` in `lockfile.rs`; `apply_uv_typeshed_override` in `basilisk-lsp::config`, called from `load_cli_workspace_config` and MCP `status_for_workspace`).
- [x] Ambiguous or absent → no auto-pin (bundled default + `typeshed_source_unpinned`).
- [x] Candidate matching uses [PEP 503](https://peps.python.org/pep-0503/#normalized-names) normalisation
      (every run of `-`, `_`, `.` folds to one `-`, then lower-case), so each spelling `uv.lock` may record
      for the recognised distribution resolves and a merely similar name does not. The recognised list is
      asserted to be stored already-normalised, or an entry could never match.
- [x] Ambiguity is refused, not resolved: one candidate package whose wheels carry **differing** digests
      yields no auto-pin, exactly as two candidate packages do — nothing in a lock file marks one wheel as
      the stdlib source, so pinning the first would make the checked stdlib depend on file ordering. The
      same digest repeated in a different case is one artifact, not two, and the pin is canonicalised to
      lower case because the store compares it against a lower-case re-hash.
- [x] Tests: `uv.lock` pinning the package → auto-pinned, no advisory; two candidates → no auto-pin; one
      candidate with differing wheel digests → no auto-pin; repeated identical digests still resolve; every
      PEP 503 spelling matches and near-misses do not; explicit source wins; no `uv.lock` → no-op
      (`lockfile.rs` × 9, `config.rs` × 4).

### Cross-cutting gates

- [x] `make test` — `_test_rust` green (all Rust unit/integration tests + conformance fixtures + every per-crate coverage threshold; `basilisk-lsp` ratcheted 86 → 87 and `basilisk-cli` measures 96 % ≥ 95 %). `_test_vsix`/`_test_nvim`/`_test_zed` each pass in isolation; under `make test`'s `-j3` parallel run the neovim E2E coverage flaps 43–45 % around its 44 % threshold (pre-existing parallel-load flakiness in the LSP e2e harness; the LSP server path is untouched by S4 — `apply_uv_typeshed_override` is wired into the CLI and MCP only, never `load_analysis_config`).
- [x] clippy + fmt at strictest (verified: `cargo clippy --workspace --all-targets` 0 errors/warnings; `cargo fmt --all --check` clean).
- [x] `make lint` dependency-shape portion (verified: `scripts/check-dependency-shape.sh` — analysis crates offline, only `basilisk-typeshed-fetch` links HTTP).
- [x] `deslop` clean (verified: `deslop .` exits 0 within the committed `.deslop.toml` budget).
- [ ] `make bench` — **FAILS on a cold-start regression (≈ +2 ms, ~6.2→9.4 ms on the fastest fixture) that this slice did not cause.** Tracked and owned OUTSIDE this plan; it does not gate S1–S4 and this plan claims no benchmark result. What is established:
  - **S4 is perf-neutral**: toggling the `apply_uv_typeshed_override` call off moves the mean within noise (8.4 ms vs 8.8 ms, σ 0.5). Its common-case cost is one `is_file()` stat. S1–S3 add only a skipped `else if` arm to `typeshed_request` on the default path.
  - **The `rustls`/`ureq` dyld hypothesis is disproven**, and any note repeating it is wrong: `basilisk-cli` already depended on `basilisk-typeshed-fetch` on `main`, so the TLS stack was linked into the binary that produced the 6.2 ms baseline.
  - **The committed baseline is stale, not just slow**: it was last written by `009f2556` (2026-07-18) while `main` has since merged through `e3e97d30` (2026-08-01, #377). Many merged PRs sit between the baseline and this branch, so nothing attributes the delta to this branch without a same-machine A/B of `main` HEAD vs this branch.
  - **Part of the delta is environmental**: the two runs pin identical competitor versions, and pyright/mypy/ty/pyrefly/zuban all shifted 3–10 % between them — real, but far short of basilisk's ~50 % on the fast fixtures, so a genuine fixed per-process cost remains to be found.
  - Recovering the cost — not re-baselining — is the exit condition, and it belongs to the benchmark task, not to this one. The benchmark itself gates nothing ([CHKARCH-TESTING-BENCH](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH)); it is read by a human, and no number here passes or fails a build.
- [x] Conformance 100 % / 0 FP unchanged (advisories never enter the scored stream; conformance fixtures ran green inside `_test_rust`).
