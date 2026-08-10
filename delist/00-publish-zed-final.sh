#!/usr/bin/env bash
# Replace the public Zed mirror with the notice-only extension.
#
# Implements [WITHDRAWAL-UNLIST] and [ZED-MIRROR].
#
# Basilisk is NOT in the Zed extension registry and never was. There is no
# `[basilisk]` block in zed-industries/extensions/extensions.toml, no
# extensions/basilisk submodule, and no commit in that repo has ever mentioned
# it — the `publish-zed` job was removed from release.yml after its registry
# step failed the v0.41.0 release, and it never landed before that. So there is
# nothing to bump and nothing to remove there, and opening a listing PR NOW
# would add Basilisk to a registry it was never in, in the middle of unlisting
# it. Do not do that. 06-unlist-zed.sh re-checks this and fails if it changes.
#
# What IS public is the mirror, Nimblesite/basilisk-zed. Anyone can read it, and
# Zed installs a dev extension straight from a local clone of exactly this
# layout. Its `main` still serves the OLD extension: a [language_servers.basilisk]
# block that launches `basilisk lsp` — a command the inert CLI no longer has —
# and a description advertising diagnostics, autocomplete, refactoring, and
# profiling. That is a live product claim for a checker that was wrong.
#
# This script replaces that tree with the notice-only extension, so the mirror
# says what every other surface says. 06-unlist-zed.sh then archives it.
#
# Needs: gh authenticated; push rights to Nimblesite/basilisk-zed; cargo with
# the wasm32-wasip2 target (the push is gated on a real standalone build).
#
#   delist/00-publish-zed-final.sh v0.41.2 [--yes]

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

VERSION="${1:-}"
[ -n "$VERSION" ] || fail "usage: 00-publish-zed-final.sh <tag, e.g. v0.41.2> [--yes]"
shift
parse_args "$@"
BARE="${VERSION#v}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_TOML="https://raw.githubusercontent.com/zed-industries/extensions/main/extensions.toml"

banner "Zed mirror — Nimblesite/basilisk-zed"

require_cmd gh "the mirror is pushed over an authenticated remote"
require_cmd git "the mirror is pushed as a clone"
require_cmd cargo "the push is gated on a standalone wasm build"
require_cmd curl "the registry is checked before anything is published"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# If a listing ever appears, every assumption above is void: publishing would
# then be updating a real listing, and the removal PR in 06 becomes necessary.
step "Confirm Basilisk is still absent from the Zed registry"
if curl -fsSL "$REGISTRY_TOML" | grep -q '^\[basilisk\]'; then
    fail "zed-industries/extensions now lists basilisk — re-read 06-unlist-zed.sh before publishing"
fi
ok "no basilisk entry in the registry; this publishes to the mirror only"

step "Render the standalone tree at $BARE"
"$REPO_ROOT/scripts/render-zed-mirror.sh" "$work/render" "$BARE"

# Gate the push on a real build. A tree that does not compile standalone is a
# broken dev extension for anyone who clones the mirror.
step "Build it standalone"
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

if confirm "replace the public Zed mirror with the notice-only extension"; then
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

    ok "mirror replaced and tagged $VERSION"
    warn "No registry PR was opened, and none should be: Basilisk is not listed on Zed."
fi
