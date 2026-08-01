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
| S4 — uv auto-detection | ⬜ Not started |
| Cross-cutting gates | 🟡 Partial (clippy/fmt/dep-shape green; `make test`, `deslop`, `make bench`, conformance pending) |

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
Store paths do not collide: 40-hex = commit, 64-hex = wheel.

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
Store layout `<store>/<64-hex sha256>/wheel.whl` + manifest. `RuntimeBackend::load_pypi_package`
reads the stored wheel, SHA-256-hashes it, asserts == pin, builds the snapshot from the wheel's
`stdlib/` via the archive VFS, identity `PyPIPackage`, no advisories. Missing → `BackendError::Missing`;
hash mismatch → `BackendError::Corrupt`; both surface as `NO SOURCE`.
**Acceptance:** verified wheel activates & suppresses; missing → `NO SOURCE` naming
`basilisk typeshed download --package`; tampered → `NO SOURCE`.
**Open item (close before S2):** confirm the target package's wheel ships a `stdlib/` tree; the
shape gate rejects otherwise. If the real layout differs, a mapping is required — flagged, not assumed.

### S3 — Segregated acquisition · `TYPESHEDPYPI-S3`
Extend `basilisk-typeshed-fetch` (the only HTTP crate): resolve `<name>` → PyPI JSON API →
select the wheel whose SHA-256 == pin → re-hash the fetched bytes (the index digest is never
trusted) → run the **Safety and Shape** gates reused from the commit download path → write
`<store>/<sha256>/wheel.whl` + manifest. The **License** and **Content** gates are intentionally
skipped: the License gate attests the build-approved *typeshed* `LICENSE` identity (a third-party
wheel ships its own license), and the Content gate reconstructs a Git root tree (a wheel is not a
Git tree) — the wheel SHA-256 *is* the content attestation for a `PyPI` source. Write nothing on
failure. CLI `basilisk typeshed download --package <name>@sha256:<hex>`.
**Acceptance:** verify+store; write-nothing-on-failure; `scripts/check-dependency-shape.sh` still
passes (only `basilisk-typeshed-fetch` links HTTP); `GITHUB_TOKEN` never sent to PyPI.

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
`scripts/check-dependency-shape.sh` — `basilisk-stubs` still links no HTTP client), `deslop`,
`make bench` (zero-tolerance baseline gate — no perf regression), conformance 100 % / 0 FP
unchanged (advisories never enter the scored stream).

## TODO {#TYPESHEDPYPI-TODO}

### S1 — Source model & config (no transport) · `TYPESHEDPYPI-S1`

- [x] `SourceSelection`/`SourceKind`/`SourceIdentity::PyPIPackage { name, sha256 }`.
- [x] Selector arm: validate identity + active-source, emit **no** advisory.
- [x] `SourceBackend::load_pypi_package` + `SelectionError`/`BackendError::PyPIPackage` (fail-closed).
- [x] `typeshed-package` parsing (TOML + JSON) + mutual exclusion with `typeshed-commit`/`typeshed-path`.
- [x] `TypeshedActiveSource::PyPIPackage` + `TypeshedSource::PyPIPackage`.
- [x] Config-editor allowlists wired (`TypeshedConfigKey`, `TypeshedSettingKey`, mutation validation, `typeshed_policy_changed`, snapshot projection).
- [x] Tests: selector pin suppresses advisories; config merge/spec-shape; mutation 3-way exclusion.

### S2 — Offline verification backend · `TYPESHEDPYPI-S2`

- [x] Store layout `<store>/<64-hex sha256>/wheel.whl` + manifest (40-hex = commit, 64-hex = wheel — no collision).
- [x] `RuntimeBackend::load_pypi_package`: read stored wheel, SHA-256-hash it, assert == pin.
- [x] Build snapshot from the wheel's `stdlib/` via the archive VFS; identity `PyPIPackage`; no advisories.
- [x] Missing → `BackendError::Missing`; hash mismatch → `BackendError::Corrupt`; both surface as `NO SOURCE`.
- [x] **Open item (resolved for the contract):** the Shape gate IS the guard — a synthetic wheel shipping `stdlib/` + root `LICENSE` passes it (`fake_wheel()` in `testing.rs`, exercised end-to-end by the S3 round-trip test). A real wheel whose layout differs would be rejected at download time and at read time; a layout *mapping* is **not** assumed — if the target package ships a different tree, that is an integration-time follow-up, not an S2 gap.
- [x] Tests: verified wheel activates & suppresses; missing → `NO SOURCE` naming `basilisk typeshed download --package`; tampered → `NO SOURCE` (`wheel.rs`, `runtime/tests.rs`, `selector/tests.rs`).

### S3 — Segregated acquisition · `TYPESHEDPYPI-S3`

- [x] Extend `basilisk-typeshed-fetch` (the only HTTP crate): `<name>` → PyPI JSON API → select wheel whose SHA-256 == pin (`pypi.rs` — `PypiApi` trait + `PypiClient`, anonymous HTTPS-only, no credential).
- [x] Re-hash fetched bytes; run Safety + Shape gates (License/Content intentionally skipped — see slice text); write `<store>/<sha256>/wheel.whl` + manifest (`download_package` in `lib.rs`, `write_wheel` in `wheel.rs`).
- [x] Write nothing on failure; `GITHUB_TOKEN` never sent to PyPI (`PypiClient` holds no credential).
- [x] CLI `basilisk typeshed download --package <name>@sha256:<hex>` (`typeshed_cli.rs` — `--package` flag, mutually exclusive with `--commit`).
- [x] Tests: verify+store round-trip via `read_snapshot`; byte-mismatch/resolve/download failures write nothing; unknown digest fails closed; `--package` CLI exit codes (0/2/3); `scripts/check-dependency-shape.sh` still passes (only `basilisk-typeshed-fetch` links HTTP).

### S4 — uv auto-detection · `TYPESHEDPYPI-S4`

- [ ] Extend `basilisk-uv` lockfile parser to capture `wheels[].hash` (currently dropped into `extra`).
- [ ] When `typeshed-package` is unset, if `uv.lock` pins exactly one recognised typeshed-distribution package, auto-resolve from its wheel hash.
- [ ] Ambiguous or absent → no auto-pin (bundled default + `typeshed_source_unpinned`).
- [ ] Tests: `uv.lock` pinning the package → auto-pinned, no advisory; two candidates → no auto-pin.

### Cross-cutting gates

- [ ] `make test` green (coverage ratchet up).
- [x] clippy + fmt at strictest (verified: `cargo clippy --workspace --all-targets` 0 errors/warnings; `cargo fmt --all --check` clean).
- [x] `make lint` dependency-shape portion (verified: `scripts/check-dependency-shape.sh` — analysis crates offline, only `basilisk-typeshed-fetch` links HTTP).
- [ ] `deslop` clean.
- [ ] `make bench` — zero-tolerance baseline gate, no perf regression.
- [ ] Conformance 100 % / 0 FP unchanged (advisories never enter the scored stream).
