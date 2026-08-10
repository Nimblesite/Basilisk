# Unlisting runbook

Implements [WITHDRAWAL-UNLIST](../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-UNLIST). Every published word comes from that spec; nothing here restates it.

## The order, and why it is not negotiable

**Publish the final version → verify it is live → unlist.**

Unlisting hides a listing. It does nothing to a copy already installed on a developer's machine — that copy keeps checking, and keeps being wrong. The only thing that reaches an existing install is a published update. So the last version shipped to every channel is the one carrying the statement and the [inert CLI](../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT), and the listing comes down straight afterwards.

Unlist first and the message never arrives.

## Scripts

Every script is **dry run by default** and prints what it would do. Pass `--yes` to act; each then asks you to type `UNLIST` before touching anything public. Run them from the repository root.

| # | Script | Does |
|---|---|---|
| 1 | `01-verify-final-release.sh v0.42.0` | Read-only. Asserts every channel is serving the final version. **Nothing below runs until this passes.** |
| 2 | `02-unlist-marketplace.sh` | `vsce unpublish` removes the extension from the VS Code Marketplace. Needs `VSCE_PAT`. |
| 3 | `03-unlist-homebrew.sh` | Deletes `Formula/basilisk.rb` from `Nimblesite/homebrew-tap`. Needs `gh`. |
| 4 | `04-unlist-scoop.sh` | Deletes `bucket/basilisk.json` from `Nimblesite/scoop-bucket`. Needs `gh`. |
| 5 | `05-unlist-nvim-mirror.sh` | Archives `Nimblesite/basilisk.nvim` (read-only, not deleted). Needs `gh`. |
| 6 | `06-unlist-zed.sh` | Opens the PR removing `basilisk` from `zed-industries/extensions`. Needs `gh`. |
| 7 | `07-unlist-github-repo.sh` | Rewrites the repo description/topics and disables the Release workflow. Needs `gh`. |
| 8 | `08-verify-unlisted.sh` | Read-only. Asks each channel's public API what it still serves. Run after, and again a day later. |

## Manual steps

These have no API that a token can drive, or they end in someone else's review queue. Do them in this order, alongside the scripts.

| Channel | What to do | Where | Done when |
|---|---|---|---|
| **PyPI — `basilisk-python`** | **Yank every release** (Manage project → Releases → each version → Options → Yank). Yank, do not delete: deleting breaks existing pinned lockfiles and destroys the record, while yanking removes the release from resolution so no new install picks it up. | https://pypi.org/manage/project/basilisk-python/releases/ | `08-verify-unlisted.sh` reports every release yanked |
| **PyPI — project description** | The project page stays, so its description must be the statement. It is set by the wheel metadata, so this is already correct if the final release published — check the rendered page. | https://pypi.org/project/basilisk-python/ | The page opens with "Basilisk is unlisted" |
| **Open VSX** | There is no unpublish in the `ovsx` CLI and no public API for it. Open an issue asking the Eclipse Foundation to remove `Nimblesite.basilisk`, stating that the extension is withdrawn; link the statement. | https://github.com/EclipseFdn/open-vsx.org/issues | The extension 404s at https://open-vsx.org/extension/Nimblesite/basilisk |
| **Zed registry** | Script 6 opens the PR; a Zed maintainer merges it. Follow it up if it stalls. | https://github.com/zed-industries/extensions/pulls | The `basilisk` entry is gone from `extensions.toml` |
| **VS Code Marketplace publisher** | If `Nimblesite` publishes nothing else, remove the publisher's marketing profile text too — the publisher page survives the extension's removal. | https://marketplace.visualstudio.com/manage/publishers/Nimblesite | The publisher page lists no Basilisk |
| **GitHub Release workflow secrets** | Revoke `VSCODE_MARKETPLACE_PAT`, `OPEN_VSX_PAT` and `BREW_SCOOP_PAT` once unlisting is done. A disabled workflow plus live publish tokens is one re-enable away from republishing. | Org Settings → Secrets and variables → Actions | The three secrets are deleted |
| **PyPI Trusted Publisher** | Remove the `pypi` trusted publisher for `Nimblesite/Basilisk` / `release.yml`, for the same reason. | https://pypi.org/manage/project/basilisk-python/settings/publishing/ | No publisher listed |
| **Search engines** | Every retired page redirects to `/` and the sitemap lists only `/`, so this resolves on its own. Optionally request re-indexing to speed it up. | Google Search Console | Old URLs resolve to the statement |
| **Third-party listings** | Awesome-lists, comparison articles, aggregator entries. Search for `basilisk-python.dev` and `Nimblesite/Basilisk` and ask each owner to remove or annotate the entry. Do not argue; link the statement. | — | Each has been contacted once |

## What must NOT be removed

- **The GitHub repository.** It stays public. Taking it down erases what happened.
- **Existing GitHub Releases.** They stay. Deleting them destroys the record and breaks pinned installs.
- **The website.** It stays, serving the statement, with every retired URL redirecting to it — including the `/errors/BSK-XXXX/` links printed by binaries already installed.
- **The internal specs, plans, and the [integrity audit](../docs/CONFORMANCE-INTEGRITY-AUDIT.md).** They are the record, not marketing.
