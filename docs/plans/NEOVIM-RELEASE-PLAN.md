# Basilisk Neovim Plugin — Release Plan

> **Spec**: `docs/specs/NEOVIM-SPEC.md` → `[NVIM-DISTRIBUTION]` (and the shared
> `docs/specs/LSP-ARCHITECTURE-SPEC.md` for binary resolution).
> **Sibling roadmap**: `docs/plans/ROADMAP-NEXT-STEPS-PLAN.md` §1 "Editor releases".
> **Companion editor releases**: `release.yml` (`vsix` / `publish-vsix` jobs for VS Code,
> `publish-homebrew` / `publish-scoop` for the core binary). Zed publish is still a gap.

This plan covers **how we ship `basilisk.nvim` to users**. It is the Neovim analogue of the
VSIX marketplace pipeline, but the distribution model is fundamentally different and that
difference drives every decision below.

---

## 1. Why Neovim is not like VS Code or Zed

| | VS Code | Zed | **Neovim** |
|---|---|---|---|
| Distribution channel | VS Code Marketplace / Open VSX | Zed extension registry | **None — plain Git repo** |
| Artifact | `.vsix` bundle | WASM bundle | **The source tree itself** |
| Versioning | `package.json` version | `extension.toml` version | **Git tags only** |
| Install | `vsce publish` → marketplace | registry PR | **plugin manager clones a repo at a tag** |

There is **no Neovim marketplace**. Users install with a plugin manager (lazy.nvim, packer,
mini.deps, vim-plug, …) — or Neovim's own built-in `vim.pack` — that does a `git clone` of a
GitHub repo and checks out a branch or tag. "Releasing" a Neovim plugin therefore means:

1. The plugin source is the head of a **standalone, installable Git repo**, and
2. **Git tags / a stable branch** mark which commits users should pin to.

This is confirmed by the official Neovim docs (`runtime/doc/{repeat,usr_05,pack}.txt`):

- **Packages load directories already on disk.** `'packpath'` + `pack/*/start` (auto-loaded) and
  `pack/*/opt` (via `:packadd`); the package dir is added to `'runtimepath'`. Core Neovim does not
  download plugins through this mechanism.
- **The repo root must be the plugin root.** The documented layout is
  `…/pack/<x>/start/<plugin>/plugin/…`, with the warning to "make sure that you end up with a path
  like this" — `plugin/`, `lua/`, `doc/` must sit at the top of the installed directory. There is
  **no install-from-subdirectory** facility — the constraint that forces the mirror in §3.
- **`helptags` are generated at install time** (`:helptags` over `doc/`), so CI need not generate
  them — only ship `doc/`.
- **`vim.pack.add()`** (Neovim's first-party manager) reinforces all of the above: its `src` is a
  Git URL ("Any format supported by `git clone`"), it clones the **whole repo** (no subdir), and
  its `version` field selects a branch/tag/commit **or a `vim.version.range()` semver constraint** —
  an extra argument for the tag-based versioning model (§5).

Optional secondary channels: **LuaRocks** (for `rocks.nvim` / `luarocks` users) and a
**nvim-lspconfig PR** (for users who want only the bare LSP, no plugin features).

---

## 2. Current state (what exists, what's missing)

**Exists**

- Full plugin at `basilisk.nvim/` — 17 Lua modules, vim help (`doc/basilisk.txt`),
  `:checkhealth basilisk`, 189 passing tests. Feature parity with VS Code/Zed reached
  (`NEOVIM-PLAN.md` Phases 1–11).
- CI test job `test-nvim` in `.github/workflows/ci.yml` (Neovim 0.11.6, real LSP e2e +
  screenshot regression). This gates *quality*, not *release*.
- Binary auto-download already targets the right place:
  `basilisk.nvim/lua/basilisk/binary.lua` → `GITHUB_REPO = "Nimblesite/Basilisk"`, pulling
  per-platform archives from `Nimblesite/Basilisk` GitHub releases (produced by the `build`
  job in `release.yml`). So the plugin's runtime dependency on a release **already works** as
  long as the monorepo tag exists.

**Missing / blocking**

- **No standalone installable repo.** The plugin lives in a subdirectory of
  `Nimblesite/Basilisk`. lazy.nvim/packer cannot install a subdirectory of a repo; they need a
  repo whose root *is* the plugin.
- **No release/tagging step for the plugin** in `release.yml` (unlike `vsix`/homebrew/scoop).
- **Naming inconsistency to resolve.** Install docs (`NEOVIM-SPEC.md [NVIM-DISTRIBUTION]`,
  `doc/basilisk.txt`, `NEOVIM-PLAN.md`) tell users to install **`basilisk-lang/basilisk.nvim`**,
  but the actual GitHub org is **`Nimblesite`** (git remote + `binary.lua`). One of these is
  wrong and every published install snippet must agree. **Decision required — see §7.**
- **No embedded version string.** `scripts/stamp-version.sh` stamps Cargo, Zed, shipwright,
  website, and VSIX, but **not the nvim plugin** (it has no version field). Tags alone identify
  versions today; `:BasiliskInfo`/health report the *binary* version, not a plugin version.

---

## 3. Mechanism — generated mirror, written like the Homebrew/Scoop publishers

Keep `basilisk.nvim/` canonical **inside the monorepo** (so it stays co-located with the LSP it
drives, per CLAUDE.md: "the LSP drives the functionality"). On each release, **publish the plugin
tree** to a dedicated standalone repo that users actually install from — using the **identical
write convention** the existing `publish-homebrew` / `publish-scoop` jobs already use for their
sibling repos.

```
Nimblesite/Basilisk           (monorepo, canonical source — basilisk.nvim/)
        │  release.yml on tag vX.Y.Z
        │  clone mirror · replace content with basilisk.nvim/ · bot commit · push + tag
        ▼
Nimblesite/basilisk.nvim      (generated mirror — users install THIS)
        ├── main  (latest published tree, "basilisk X.Y.Z" bot commit)
        └── vX.Y.Z tag (matches monorepo) → version-pinned installs
```

**Write convention (same as `publish-homebrew` / `publish-scoop`)** — `git clone` the sibling
`Nimblesite/*` repo with `https://x-access-token:${BREW_SCOOP_PAT}@…`, write the content, then:

```sh
git config user.name  "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add -A
git diff --cached --quiet || git commit -m "basilisk ${BASILISK_VERSION}"   # skip if unchanged
git push
```

The only differences from the brew/scoop jobs: the *content* written is the whole `basilisk.nvim/`
tree (not a single rendered formula/manifest), and the job **also pushes a `vX.Y.Z` tag** — plugins
are git-tag versioned, so pinned installs (`vim.pack` / lazy.nvim) need it; a formula/manifest does
not.

**Why a generated mirror**

- Single source of truth — no drift between monorepo and published plugin (CLAUDE.md: avoid
  duplication of all kinds).
- The tag on the mirror **matches** the monorepo tag, so `binary.lua`'s auto-download (which
  resolves the *binary* from `Nimblesite/Basilisk` releases) and the *plugin* version are
  always the same number. A user on plugin `v0.5.0` gets binary `v0.5.0`.

**Why not install-from-subdir**: lazy.nvim/packer/vim-plug (and `vim.pack`) do not support pinning
to a subdirectory of a repo; the repo root must be the runtimepath entry. A mirror is the minimal,
robust way to satisfy that without restructuring the monorepo.

**Resulting install snippets** (publish once the mirror exists; keep all in sync with §7.1):

```lua
-- lazy.nvim
{ 'Nimblesite/basilisk.nvim', ft = 'python',
  dependencies = { 'mfussenegger/nvim-dap' } }  -- optional

-- vim.pack (built-in, Neovim 0.12+) — config-free, no third-party manager
vim.pack.add({
  { src = 'https://github.com/Nimblesite/basilisk.nvim',
    version = vim.version.range('*') },  -- latest stable tag; or pin 'v0.5.0'
})
require('basilisk').setup({})
```

---

## 4. Release pipeline (new `publish-nvim` job in `release.yml`)

Trigger: same `on: push: tags: ['v*']` as the rest of `release.yml`. Run **after** the
`github-release` job (so the binaries the plugin downloads already exist for that tag), mirroring
how `publish-homebrew`/`publish-scoop` declare `needs: github-release`.

Steps (a plain `actions/checkout@v4` — no `fetch-depth: 0`, since we publish the *tree*, not
rewritten history):

1. Clone the mirror with the `x-access-token` credential, exactly as the brew/scoop jobs clone
   their tap/bucket.
2. Replace the mirror's tracked content with the current `basilisk.nvim/` tree (preserve `.git`):
   `find mirror -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +` then `cp -R basilisk.nvim/. mirror/`.
3. `git config` the `github-actions[bot]` identity, `git add -A`, skip-if-`git diff --cached
   --quiet`, else `git commit -m "basilisk ${BASILISK_VERSION}"` and `git push` — byte-for-byte the
   brew/scoop convention.
4. Push the `vX.Y.Z` tag (idempotent — skip if it already exists on the mirror). This is the one
   nvim-specific step; the formula/manifest publishers don't tag.
5. `helptags` are **not** generated in CI — plugin managers run `:helptags` on install. Just ensure
   `doc/basilisk.txt` ships in the tree (it does).

**Secrets**: **reuse the existing `BREW_SCOOP_PAT`** — `publish-homebrew` and `publish-scoop`
already use it to push to sibling `Nimblesite/*` repos (`homebrew-tap`, `scoop-bucket`) via the
`x-access-token` credential, which is the identical mechanism `publish-nvim` needs to push to
`Nimblesite/basilisk.nvim`. The only requirement is that the token can **write to
`Nimblesite/basilisk.nvim`**: a classic `repo`-scoped PAT covers it automatically; a fine-grained
PAT must have `basilisk.nvim` added to its allowed repositories (Contents: read/write). The job
carries the same failure-with-actionable-message guard as the other two.

**No new tooling**: the job uses only `git`, `find`, and `cp` — already present on the runner and
identical to the brew/scoop jobs — so no `ci.yml`/Dockerfile dependency changes are needed.

---

## 5. Versioning decision for the plugin

Two viable models — pick one in §7:

- **(A) Tag-only (recommended, lowest friction).** No version string in the plugin. The mirror's
  tag *is* the version. `:BasiliskInfo`/health continue to report the binary version (which now
  always matches the tag). No `stamp-version.sh` change. Simplest; matches how most Neovim
  plugins work.
- **(B) Embedded version.** Add a `basilisk.nvim/lua/basilisk/version.lua` (or a field read by
  `info.lua`/`health.lua`) carrying `0.0.0-PLACEHOLDER`, and add it to `stamp-version.sh`'s
  `FILES` list so the release stamps it. Needed only if we later publish a **rockspec**
  (LuaRocks requires a version) or want the plugin to self-report a version independent of the
  binary. Defer until LuaRocks is on the table (§6).

---

## 6. Secondary channels (optional, do after §3–§4 land)

- **LuaRocks** (`luarocks.org`, consumed by `rocks.nvim`): add a `basilisk.nvim-X.Y.Z-1.rockspec`
  and a `publish-nvim-luarocks` step using `luarocks upload` with a `LUAROCKS_API_KEY` secret.
  Requires version model **(B)**. The roadmap explicitly marks LuaRocks **optional**
  (`ROADMAP-NEXT-STEPS-PLAN.md` §1). Low priority.
- **nvim-lspconfig PR** (`[NVIM-DISTRIBUTION-SECONDARY-LSPCONFIG-PR]`, `NEOVIM-PLAN.md` Phase 9):
  one-time, manual, human PR submitting the bare `lsp/basilisk.lua` to `neovim/nvim-lspconfig`.
  Not part of CI. Tracked separately; gives "just the LSP" users a path without the full plugin.

---

## 7. Decisions required before implementation

1. **Mirror repo name/org.** Resolve the `basilisk-lang/basilisk.nvim` vs `Nimblesite/basilisk.nvim`
   inconsistency. Recommended: **`Nimblesite/basilisk.nvim`** — matches the existing
   `Nimblesite/Basilisk` remote and `binary.lua`'s `GITHUB_REPO`. Whatever is chosen, update **all
   three** install snippets: `NEOVIM-SPEC.md [NVIM-DISTRIBUTION-PRIMARY-STANDALONE]`,
   `doc/basilisk.txt` (lazy + packer blocks), and `NEOVIM-PLAN.md` Phase 9. (`binary.lua`'s
   `Nimblesite/Basilisk` is the *binary* source and stays as-is.)
2. **Versioning model** — (A) tag-only vs (B) embedded version (§5). Recommended: **(A)** now,
   revisit at LuaRocks time.
3. **Write mechanism** — resolved: **match `publish-homebrew` / `publish-scoop`** (clone the
   mirror, replace content, `github-actions[bot]` commit, push) rather than a `git subtree split`.
   This reuses the established convention and `BREW_SCOOP_PAT`, needs no extra tooling, and the
   generated mirror does not need rewritten per-commit history.

---

## 8. Task list

> Decisions taken for this implementation: §7.1 → **`Nimblesite/basilisk.nvim`**,
> §7.2 → **(A) tag-only**, §7.3 → **match the brew/scoop write convention** (`BREW_SCOOP_PAT` +
> `github-actions[bot]` commit + push, plus an nvim-only `vX.Y.Z` tag).

- [x] **`[HUMAN]`** Create the mirror repo `Nimblesite/basilisk.nvim` (created; seeded once
      manually from `main` to validate install).
- [ ] **`[HUMAN]`** Ensure the existing `BREW_SCOOP_PAT` can write to `Nimblesite/basilisk.nvim`
      (classic `repo` PAT: automatic; fine-grained: add `basilisk.nvim` to its allowed repos). No
      new secret needed — reuses the homebrew/scoop token.
- [x] Fix the org-name inconsistency in `NEOVIM-SPEC.md`, `doc/basilisk.txt`, `NEOVIM-PLAN.md`
      (§7.1). Spec edit keeps the `[NVIM-DISTRIBUTION-*]` IDs.
- [x] Add a **`vim.pack.add()`** install snippet (§3) alongside the lazy.nvim/packer blocks in
      `NEOVIM-SPEC.md [NVIM-DISTRIBUTION-PRIMARY-STANDALONE]` and `doc/basilisk.txt` — Neovim's
      built-in, no-third-party-manager install path (0.12+).
- [x] Add a `[NVIM-DISTRIBUTION-RELEASE]` subsection to `NEOVIM-SPEC.md` describing the
      mirror mechanism, and cross-reference this plan.
- [x] Add the `publish-nvim` job to `release.yml` (§4), `needs: github-release`, written with the
      same clone / `github-actions[bot]` commit / push convention as the homebrew/scoop jobs.
- [x] Model **(A) tag-only** chosen (§5) — no `version.lua`, no `stamp-version.sh` change.
- [ ] **`[HUMAN/CI]`** Dry-run on the next tag: confirm the mirror gets the `basilisk ${VERSION}`
      bot commit on `main` plus the matching `vX.Y.Z` tag. (Requires `BREW_SCOOP_PAT` write access.)
- [ ] **`[HUMAN]`** Clean-machine smoke test (`ROADMAP-NEXT-STEPS-PLAN.md` §1 sign-off): on a
      fresh box, `lazy.nvim` install the *published mirror tag*, open a real Python project,
      confirm zero-config binary resolution/auto-download, diagnostics, hover, go-to-def, debug
      (nvim-dap), and profiling all light up. This is a release gate.
- [x] Mark `NEOVIM-PLAN.md` Phase 9 release-mechanism item done; link this plan.
- [ ] **`[LATER]`** LuaRocks rockspec + `publish-nvim-luarocks` step (§6) — optional.
- [ ] **`[LATER]`** nvim-lspconfig PR (§6) — manual, one-time.

---

## 9. Verification & rollback

- **Verification**: the next tagged release exercises the pipeline end-to-end — confirm the mirror
  receives the `basilisk ${VERSION}` bot commit and the matching `vX.Y.Z` tag. The clean-machine
  smoke test proves a real user install works against the *published* artifact, not the dev tree.
- **Rollback**: the mirror is generated, so a bad release is corrected by tagging a fixed
  `vX.Y.Z+1` from the monorepo and re-running `publish-nvim` (the bot commit + new tag supersede
  it; a bad tag can be deleted on the mirror). Because the plugin and binary versions are
  tag-coupled (§3), there is no partial-release state where the plugin expects a binary that does
  not exist.
