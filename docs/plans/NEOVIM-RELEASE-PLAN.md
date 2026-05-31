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
mini.deps, vim-plug, …) that does a `git clone` of a GitHub repo and checks out a branch or
tag. "Releasing" a Neovim plugin therefore means:

1. The plugin source is the head of a **standalone, installable Git repo**, and
2. **Git tags / a stable branch** mark which commits users should pin to.

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

## 3. Recommended mechanism — subtree mirror, tag-synced to the monorepo

Keep `basilisk.nvim/` canonical **inside the monorepo** (so it stays co-located with the LSP it
drives, per CLAUDE.md: "the LSP drives the functionality"). On each release, **publish a
read-only split** of that subdirectory to a dedicated standalone repo that users actually
install from.

```
Nimblesite/Basilisk           (monorepo, canonical source)
        │  release.yml on tag vX.Y.Z
        │  git subtree split --prefix=basilisk.nvim
        ▼
Nimblesite/basilisk.nvim      (generated, read-only mirror — users install THIS)
        ├── pushed to `main` (rolling)        → users on HEAD get latest
        └── tagged `vX.Y.Z` (matches monorepo) → users pinned to a release
```

**Why a mirror and not a separate hand-maintained repo**

- Single source of truth — no drift between monorepo and published plugin (CLAUDE.md: avoid
  duplication of all kinds).
- The tag on the mirror **matches** the monorepo tag, so `binary.lua`'s auto-download (which
  resolves the *binary* from `Nimblesite/Basilisk` releases) and the *plugin* version are
  always the same number. A user on plugin `v0.5.0` gets binary `v0.5.0`.
- Standard, well-trodden pattern (`git subtree split`, or `splitsh-lite` for speed).

**Why not install-from-subdir**: lazy.nvim/packer/vim-plug do not support pinning to a
subdirectory of a repo; the repo root must be the runtimepath entry. A mirror is the minimal,
robust way to satisfy that without restructuring the monorepo.

---

## 4. Release pipeline (new `publish-nvim` job in `release.yml`)

Trigger: same `on: push: tags: ['v*']` as the rest of `release.yml`. Run **after** the
`github-release` job (so the binaries the plugin downloads already exist for that tag), mirroring
how `publish-homebrew`/`publish-scoop` declare `needs: github-release`.

Steps:

1. `actions/checkout@v4` with **`fetch-depth: 0`** (subtree split needs full history).
2. Produce the split commit of `basilisk.nvim/`:
   - Simple: `git subtree split --prefix=basilisk.nvim -b nvim-release`.
   - Faster on large histories: `splitsh-lite --prefix=basilisk.nvim`.
3. (If §7 chooses to embed a version) run a small stamp step against the split — see §5.
4. Push to the mirror and tag it, using a PAT with write access to the mirror repo:
   - `git push --force <mirror> nvim-release:main`
   - `git push <mirror> <split-sha>:refs/tags/${GITHUB_REF_NAME}`
   - Mark prerelease tags (`*-alpha`, `-rc.N`) consistently with the rest of `release.yml`
     (`contains(github.ref_name, '-')`); the mirror's `main` should track the latest **stable**
     tag, not prereleases.
5. Generate `helptags` is **not** needed in CI — plugin managers run `:helptags` on install. Do
   verify `doc/basilisk.txt` and `doc/tags` ship in the split.

**Secrets** (store as org/repo Actions secrets, matching the homebrew/scoop convention which use
`BREW_SCOOP_PAT`): a `NVIM_MIRROR_PAT` (fine-grained PAT scoped to `Nimblesite/basilisk.nvim`,
contents:write). Add the failure-with-actionable-message guard the other publish jobs use
("secret not accessible → add repo to org allow-list").

**Keep CI/Dockerfile in sync** (CLAUDE.md): if the job adds tooling (e.g. `splitsh-lite`), reflect
it where dependencies are pinned.

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
3. **Split tooling** — `git subtree split` (zero deps) vs `splitsh-lite` (faster, extra tool).
   Recommended: start with `git subtree split`; switch only if CI time becomes a problem.

---

## 8. Task list

- [ ] **`[HUMAN]`** Resolve decision §7.1 (mirror org/name) and create the empty mirror repo.
- [ ] **`[HUMAN]`** Create `NVIM_MIRROR_PAT` (fine-grained, contents:write on the mirror) and add
      it to the repo/org Actions secrets allow-list.
- [ ] Fix the org-name inconsistency in `NEOVIM-SPEC.md`, `doc/basilisk.txt`, `NEOVIM-PLAN.md`
      (§7.1). Spec edit must keep the `[NVIM-DISTRIBUTION-*]` IDs.
- [ ] Add a `[NVIM-DISTRIBUTION-RELEASE]` subsection to `NEOVIM-SPEC.md` describing the
      subtree-mirror mechanism, and cross-reference this plan (per CLAUDE.md: every spec section
      gets a non-numeric hierarchical ID; code/CI references it).
- [ ] Add the `publish-nvim` job to `release.yml` (§4), `needs: github-release`, comment it the
      same way the homebrew/scoop jobs are commented.
- [ ] (If model B) add `version.lua` + extend `scripts/stamp-version.sh` `FILES` (§5).
- [ ] Dry-run on a prerelease tag (e.g. `vX.Y.Z-rc.1`): confirm the mirror gets the tag but
      `main` does **not** advance to a prerelease.
- [ ] **`[HUMAN]`** Clean-machine smoke test (`ROADMAP-NEXT-STEPS-PLAN.md` §1 sign-off): on a
      fresh box, `lazy.nvim` install the *published mirror tag*, open a real Python project,
      confirm zero-config binary resolution/auto-download, diagnostics, hover, go-to-def, debug
      (nvim-dap), and profiling all light up. This is a release gate.
- [ ] Mark `NEOVIM-PLAN.md` Phase 9 "tagging/release mechanism" item done; link this plan.
- [ ] **`[LATER]`** LuaRocks rockspec + `publish-nvim-luarocks` step (§6) — optional.
- [ ] **`[LATER]`** nvim-lspconfig PR (§6) — manual, one-time.

---

## 9. Verification & rollback

- **Verification**: the dry-run prerelease tag (§8) proves the pipeline end-to-end without
  affecting stable users on the mirror's `main`. The clean-machine smoke test proves a real user
  install works against the *published* artifact, not the dev tree.
- **Rollback**: the mirror is generated and force-pushable; a bad release is corrected by tagging
  a fixed `vX.Y.Z+1` from the monorepo and re-running `publish-nvim`. Because the plugin and
  binary versions are tag-coupled (§3), there is no partial-release state where the plugin
  expects a binary that does not exist.
