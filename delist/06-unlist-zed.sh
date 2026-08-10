#!/usr/bin/env bash
# Archive the Zed extension mirror.
#
# Implements [WITHDRAWAL-UNLIST] and [ZED-MIRROR].
#
# This script used to open a PR removing `basilisk` from zed-industries/extensions.
# There is nothing there to remove. Basilisk is not in the Zed registry and never
# was: no `[basilisk]` block in extensions.toml, no extensions/basilisk submodule,
# and no commit in that repo has ever mentioned it. The `publish-zed` job was
# removed from release.yml after its registry step failed the v0.41.0 release,
# and it had not landed before that. A removal PR would ask a maintainer of
# someone else's repo to delete an entry that does not exist.
#
# The mirror, Nimblesite/basilisk-zed, IS the listing: it is public, and Zed
# installs a dev extension straight from a clone of that layout. 00 replaces its
# contents with the notice-only extension; this archives it. Archived rather
# than deleted, for the same reason as the Neovim mirror — deleting breaks every
# pinned clone and erases the record, while archiving is read-only and visibly
# dead.
#
# Needs: gh, authenticated with admin access to Nimblesite/basilisk-zed.
#
#   delist/06-unlist-zed.sh [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
parse_args "$@"
banner "Zed extension mirror — Nimblesite/basilisk-zed"

require_cmd gh "the repo is edited through the GitHub API"
require_cmd curl "the registry is re-checked before archiving"

REGISTRY_TOML="https://raw.githubusercontent.com/zed-industries/extensions/main/extensions.toml"

# Re-checked rather than assumed. If a listing ever appears, archiving the
# mirror strands it — the registry entry points at a submodule of this repo —
# and a removal PR becomes the right move after all.
step "Confirm Basilisk is absent from the Zed registry"
if curl -fsSL "$REGISTRY_TOML" | grep -q '^\[basilisk\]'; then
    fail "zed-industries/extensions now lists basilisk — open a removal PR there BEFORE archiving the mirror"
fi
ok "no basilisk entry in the registry — nothing to remove there"

step "Confirm the mirror carries the statement"
manifest="$(curl -fsSL https://raw.githubusercontent.com/Nimblesite/basilisk-zed/main/extension.toml 2>/dev/null || echo "")"
case "$manifest" in
    *"[language_servers"*) warn "the mirror still declares a language server — run 00-publish-zed-final.sh first" ;;
    *"unlisted"*) ok "the mirror manifest already carries the statement" ;;
    *) warn "could not read the mirror manifest — check it by hand before archiving" ;;
esac

if confirm "archive Nimblesite/basilisk-zed (read-only, permanent-ish)"; then
    act gh repo edit Nimblesite/basilisk-zed \
        --description "Basilisk's type checker produced incorrect results. Basilisk is unlisted and is being rebuilt from the ground up as a new product." \
        --homepage "https://www.basilisk-python.dev"
    act gh repo archive Nimblesite/basilisk-zed --yes
    ok "archived — confirm at https://github.com/Nimblesite/basilisk-zed"
fi
