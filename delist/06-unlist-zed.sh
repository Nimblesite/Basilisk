#!/usr/bin/env bash
# Open the PR that removes Basilisk from the Zed extension registry.
#
# Implements [WITHDRAWAL-UNLIST]. The Zed registry is zed-industries/extensions,
# a repo we do not own: the entry is a `[basilisk]` block in extensions.toml
# plus a git submodule. Removing it is a pull request, so this script prepares
# and opens that PR — a human on their side merges it.
#
# Needs: gh, authenticated; a fork of zed-industries/extensions is created if
# one does not exist.
#
#   delist/06-unlist-zed.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "Zed extension registry — zed-industries/extensions"

require_cmd gh "the PR is opened through the GitHub API"
require_cmd git "the registry is edited as a clone"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
branch="remove-basilisk"

body="Please remove the \`basilisk\` extension from the registry.

Basilisk's type checker was producing incorrect results. We asked for it to be
removed from the python/typing conformance results, and it has been removed
(https://github.com/python/typing/pull/2330). The code responsible is not
isolated to a known set of rules, so we cannot say how many rules are affected.
A code-quality tool that does not produce correct results is worse than useless,
so Basilisk is being unlisted from every distribution channel and its CLI is
inert — the extension can no longer start a language server.

Full statement: https://www.basilisk-python.dev/"

if confirm "open a PR removing basilisk from zed-industries/extensions"; then
    act gh repo fork zed-industries/extensions --clone=false --remote=false
    act gh repo clone zed-industries/extensions "$work/extensions" -- --depth 1
    act git -C "$work/extensions" checkout -b "$branch"
    act git -C "$work/extensions" submodule deinit -f extensions/basilisk
    act git -C "$work/extensions" rm -f extensions/basilisk
    act python3 "$(dirname "${BASH_SOURCE[0]}")/remove_registry_entry.py" "$work/extensions/extensions.toml" basilisk
    act git -C "$work/extensions" commit -am "Remove basilisk"
    act git -C "$work/extensions" push --set-upstream "$(gh api user --jq .login)" "$branch"
    act gh pr create --repo zed-industries/extensions \
        --title "Remove basilisk" --body "$body" --head "$branch"
    ok "PR opened — track it until merged, then confirm the extension is gone from Zed's registry"
fi
