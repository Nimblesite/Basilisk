#!/usr/bin/env bash
# Unpublish the extension from the VS Code Marketplace.
#
# Implements [WITHDRAWAL-UNLIST]. `vsce unpublish` removes the extension from
# the gallery entirely: it stops appearing in search and in the web listing, and
# no new install can find it. Copies already installed are NOT removed — that is
# what the final notice-only version is for, so run 01-verify-final-release.sh
# first.
#
# Needs: VSCE_PAT (Azure DevOps PAT, scope Marketplace → Manage).
#
#   delist/02-unlist-marketplace.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "VS Code Marketplace — Nimblesite.basilisk"

require_cmd npx "vsce runs through npx"
require_env VSCE_PAT "mint one at https://aka.ms/vscodepat (Marketplace → Manage)"

if confirm "unpublish Nimblesite.basilisk from the VS Code Marketplace"; then
    act npx --yes @vscode/vsce unpublish --pat "$VSCE_PAT" Nimblesite.basilisk --force
    ok "unpublished — confirm at https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk (expect 404)"
fi
