#!/usr/bin/env bash
# Remove the Basilisk manifest from the Scoop bucket.
#
# Implements [WITHDRAWAL-UNLIST]. Deleting bucket/basilisk.json makes
# `scoop install nimblesite/basilisk` fail to resolve, and stops Scoop's
# autoupdate from ever fetching another version.
#
# Needs: gh, authenticated with write access to Nimblesite/scoop-bucket.
#
#   delist/04-unlist-scoop.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "Scoop bucket — Nimblesite/scoop-bucket"

require_cmd gh "the bucket is edited through the GitHub API"
require_cmd git "the bucket is edited as a clone"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if confirm "delete bucket/basilisk.json from Nimblesite/scoop-bucket"; then
    act gh repo clone Nimblesite/scoop-bucket "$work/bucket" -- --depth 1
    act git -C "$work/bucket" rm -q bucket/basilisk.json
    act git -C "$work/bucket" commit -m "Remove basilisk: unlisted"
    act git -C "$work/bucket" push
    ok "manifest removed — confirm with: scoop search basilisk (expect no result)"
fi
