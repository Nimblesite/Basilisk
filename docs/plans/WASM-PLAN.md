# Basilisk WASM — remaining work

Shipped behaviour is in [WASM-SPEC.md](../specs/WASM-SPEC.md). This plan holds
only what is *not* built. Delete it when the acceptance gate below passes.

Motivating issue: [#323 — Playground](https://github.com/Nimblesite/Basilisk/issues/323).

## Shipped

The `basilisk-wasm` crate: the four-stage pipeline, the embedded typeshed, the
CLI-identical JSON contract, and host-side tests ([WASM-TESTING]).

## 1. CI build job and size ratchet {#WASM-PLAN-CI}

The crate is not yet built for `wasm32-unknown-unknown` in CI, so nothing stops
a native-only dependency from silently entering the graph and breaking the
browser build.

- Add a `wasm32-unknown-unknown` build of `basilisk-wasm` to `ci.yml`, beside
  the existing `wasm32-wasip2` Zed job, which already establishes the toolchain
  and cache pattern.
- Record the `.wasm` byte size and ratchet it downward-only, matching the repo's
  other gates ([CHKARCH-TESTING-BENCH-RATCHET]). The current unoptimised release
  artefact is **7.2 MB**, of which the embedded typeshed ZIP is 2.8 MB. That is
  the honest starting point; `opt-level = "z"`, `wasm-opt -Oz` and transport
  compression have not been applied yet and should move it before the first
  baseline is frozen.
- Lazy-load the module so the page renders before a multi-megabyte download
  arrives — a size that large is a UX decision, not just a number in a gate.
- Verify the `-zstack-size` link argument holds against the deepest conformance
  fixtures ([WASM-BUILD]). It is set but only proven to link, not yet proven
  sufficient under a real wasm runtime; a stack overflow aborts the module
  instead of unwinding, so a file that blows it is a release blocker, not a
  known issue.

**Gate:** CI fails when the wasm build breaks or the artefact grows.

## 2. Multi-file support via an in-memory VFS {#WASM-PLAN-VFS}

[WASM-LIMITS] restricts a call to one file because import resolution reaches
`std::fs` directly. A playground with more than one tab needs that behind a
trait.

The coupling is concentrated, so this is an extraction rather than a rewrite:
`imports/fs_cache.rs` (the directory-listing cache), `imports/resolve.rs`, and
`exports.rs`.

- Introduce a source-provider trait for directory listing and file reading.
- Keep the native implementation byte-identical in behaviour — this must not
  move a single conformance result. The conformance suite is the gate.
- Supply an in-memory implementation for wasm and extend the API to accept a
  set of named sources.

**Gate:** a two-file playground program resolves its own imports, with
conformance still 100% / 0 false positives.

## 3. Playground site {#WASM-PLAN-SITE}

The UI the issue actually asks for. Static, on the existing Eleventy →
GitHub Pages site ([WEBSITE-E2E-SPEC.md](../specs/WEBSITE-E2E-SPEC.md)); no
server component, which is the property [WASM-NOFS] exists to preserve.

- Editor with diagnostics as inline squiggles, hovering the rule's message.
- Each diagnostic links to its generated `/errors/BSK-XXXX/` page
  ([WEBSITE-ERROR-PAGES](../specs/WEBSITE-ERROR-PAGES-SPEC.md)) — the
  playground should teach, like every other diagnostic surface.
- Share-by-URL, with the source compressed into the hash fragment so no
  server stores anything.
- Lazy-load the wasm after first paint; the page must render and be readable
  before a multi-megabyte module arrives.
- Playwright coverage in the existing website E2E suite.

**Gate:** the playground checks a program end-to-end in a real browser, in CI.

## Acceptance

All three gates pass, `basilisk-wasm` builds in CI under a size ratchet, and
the playground is live on the site. Then delete this plan.
