#!/usr/bin/env bash
# Point the GitHub repository itself at the statement, and stop it releasing.
#
# Implements [WITHDRAWAL-UNLIST]. The repo STAYS PUBLIC — taking it down would
# erase what happened — but its description, topics and website are a listing
# like any other, and the Release workflow must not be able to publish again
# after the final version.
#
# Needs: gh, authenticated with admin access to Nimblesite/Basilisk.
#
#   delist/07-unlist-github-repo.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "GitHub repository — Nimblesite/Basilisk"

require_cmd gh "the repo is edited through the GitHub API"

line="$(python3 -c '
import sys; sys.path.insert(0, "scripts")
from gen_withdrawal_copy import copy_blocks
print(copy_blocks().line)
')"

if confirm "rewrite the repo description/topics and disable the Release workflow"; then
    act gh repo edit Nimblesite/Basilisk \
        --description "$line" \
        --homepage "https://www.basilisk-python.dev"
    # Topics are a discovery surface. Every one of them advertised the checker.
    act gh api -X PUT "repos/Nimblesite/Basilisk/topics" -f "names[]=unlisted"
    # No further releases ([WITHDRAWAL-UNLIST]). Disabling beats deleting the
    # workflow: the file stays as the record of what shipped last.
    act gh workflow disable "Release" --repo Nimblesite/Basilisk
    ok "repository listing updated and the Release workflow disabled"
fi
