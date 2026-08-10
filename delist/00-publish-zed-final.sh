#!/usr/bin/env bash
# Publish the FINAL Zed extension — run this BEFORE 01-verify-final-release.sh.
#
# Implements [WITHDRAWAL-UNLIST] and [ZED-MIRROR]. Zed is the one channel the
# Release workflow does not publish: the `publish-zed` job was removed from
# release.yml after its registry-listing step failed the v0.41.0 release, so
# every other channel ships from the tag and Zed ships from here, by hand.
#
# Why it still has to ship. Zed users are not reached by the CLI release: their
# extension downloads the binary itself, so once the final binary is inert their
# editor shows "language server failed to start" and never shows the statement.
# The final extension is what replaces that with the statement — it registers no
# language server at all and prints the notice under `/basilisk`.
#
# Two things happen here, in order:
#   1. push + tag the rendered tree to Nimblesite/basilisk-zed (the mirror)
#   2. open the PR bumping `basilisk` in zed-industries/extensions to that tag
#
# Step 2 lands in someone else's review queue. Until it merges, Zed serves the
# previous version — so `06-unlist-zed.sh` (the removal PR) waits for it.
#
# Needs: gh authenticated; push rights to Nimblesite/basilisk-zed; cargo with
# the wasm32-wasip2 target (the push is gated on a real standalone build).
#
#   delist/00-publish-zed-final.sh v0.42.0 [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

VERSION="${1:-}"
[ -n "$VERSION" ] || fail "usage: 00-publish-zed-final.sh <tag, e.g. v0.42.0> [--yes]"
shift
parse_args "$@"
BARE="${VERSION#v}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

banner "Zed extension — Nimblesite/basilisk-zed + zed-industries/extensions"

require_cmd gh "the registry PR is opened through the GitHub API"
require_cmd git "the mirror is pushed as a clone"
require_cmd cargo "the push is gated on a standalone wasm build"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

step "Render the standalone tree at $BARE"
"$REPO_ROOT/scripts/render-zed-mirror.sh" "$work/render" "$BARE"

# Gate the push on the same build the registry will run. A tree that does not
# compile standalone is a listing that fails on their CI, not ours.
step "Build it exactly as the registry will"
( cd "$work/render" && cargo build --release --target wasm32-wasip2 )
ok "standalone wasm build passed"

# The notice-only contract, checked against the artefact that is about to be
# published rather than against the working tree ([ZED-NOW]).
step "Verify the rendered manifest ships no checker"
for forbidden in "[language_servers" "[debug_adapters" "[grammars"; do
    if grep -qF "$forbidden" "$work/render/extension.toml"; then
        fail "rendered extension.toml still declares ${forbidden}...] — do not publish"
    fi
done
grep -q "Basilisk is unlisted" "$work/render/src/withdrawal_notice.txt" ||
    fail "the rendered tree carries no withdrawal notice"
ok "no language server, no debug adapter, no grammar; the notice is present"

if confirm "publish the final Zed extension and open the registry bump PR"; then
    step "Push the mirror"
    # render-zed-mirror.sh replaces the clone's tracked content and preserves
    # its .git, so the mirror keeps its history rather than being force-reset.
    act git clone "https://github.com/Nimblesite/basilisk-zed.git" "$work/mirror"
    act "$REPO_ROOT/scripts/render-zed-mirror.sh" "$work/mirror" "$BARE"
    act git -C "$work/mirror" add -A
    act git -C "$work/mirror" commit -m "basilisk $BARE"
    act git -C "$work/mirror" push
    act git -C "$work/mirror" tag "$VERSION"
    act git -C "$work/mirror" push origin "$VERSION"

    step "Open the registry bump PR"
    act python3 "$REPO_ROOT/scripts/publish_zed_registry.py" "$BARE" "$VERSION"

    ok "mirror pushed and tagged $VERSION; bump PR opened"
    warn "Zed still serves the PREVIOUS version until a maintainer merges that PR."
    warn "Do not run 06-unlist-zed.sh until it is merged and live."
fi
