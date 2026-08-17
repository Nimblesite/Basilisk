#!/usr/bin/env bash
# Remove the Basilisk formula from the Homebrew tap.
#
# Implements [WITHDRAWAL-UNLIST]. Deleting Formula/basilisk.rb makes
# `brew install nimblesite/tap/basilisk` fail to resolve. Machines that already
# installed it keep the binary — which is the inert one, after the final
# release.
#
# Needs: gh, authenticated with write access to Nimblesite/homebrew-tap.
#
#   delist/03-unlist-homebrew.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "Homebrew tap — Nimblesite/homebrew-tap"

require_cmd gh "the tap is edited through the GitHub API"
require_cmd git "the tap is edited as a clone"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if confirm "delete Formula/basilisk.rb from Nimblesite/homebrew-tap"; then
    act gh repo clone Nimblesite/homebrew-tap "$work/tap" -- --depth 1
    act git -C "$work/tap" rm -q Formula/basilisk.rb
    act git -C "$work/tap" commit -m "Remove basilisk: unlisted"
    act git -C "$work/tap" push
    ok "formula removed — confirm with: brew install nimblesite/tap/basilisk (expect 'No available formula')"
fi
