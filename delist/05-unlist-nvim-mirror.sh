#!/usr/bin/env bash
# Archive the Neovim plugin mirror.
#
# Implements [WITHDRAWAL-UNLIST]. The mirror repo IS the plugin listing: plugin
# managers install straight from it. It is archived rather than deleted —
# deleting it breaks every lockfile that pins a commit and erases the record,
# while archiving makes it read-only and visibly dead. Its README is the
# statement, pushed by the final release.
#
# Needs: gh, authenticated with admin access to Nimblesite/basilisk.nvim.
#
#   delist/05-unlist-nvim-mirror.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "Neovim plugin mirror — Nimblesite/basilisk.nvim"

require_cmd gh "the repo is edited through the GitHub API"

readme_head="$(curl -fsSL https://raw.githubusercontent.com/Nimblesite/basilisk.nvim/main/README.md 2>/dev/null | head -1 || echo "")"
case "$readme_head" in
    *"unlisted"*) ok "the mirror README already carries the statement" ;;
    *) warn "the mirror README does not start with the statement ('$readme_head') — publish the final release first" ;;
esac

if confirm "archive Nimblesite/basilisk.nvim (read-only, permanent-ish)"; then
    act gh repo edit Nimblesite/basilisk.nvim \
        --description "Basilisk's type checker produced incorrect results. Basilisk is unlisted and is being rebuilt from the ground up as a new product." \
        --homepage "https://www.basilisk-python.dev"
    act gh repo archive Nimblesite/basilisk.nvim --yes
    ok "archived — confirm at https://github.com/Nimblesite/basilisk.nvim"
fi
