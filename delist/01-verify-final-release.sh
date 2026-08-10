#!/usr/bin/env bash
# Verify the FINAL version is live on every channel — run before unlisting anything.
#
# Implements [WITHDRAWAL-UNLIST]. Unlisting hides a listing; it does nothing for
# a copy already installed. The only thing that reaches an existing install is a
# published update, so the order is: publish the final version, PROVE it is live
# here, then unlist. Running the unlisting scripts before this one passes leaves
# every existing user on the last checking build, permanently.
#
# Read-only: this script publishes nothing and removes nothing.
#
#   delist/01-verify-final-release.sh v0.42.0

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

VERSION="${1:-}"
[ -n "$VERSION" ] || fail "usage: 01-verify-final-release.sh <tag, e.g. v0.42.0>"
BARE="${VERSION#v}"

require_cmd curl "the channel checks are plain HTTP"
require_cmd python3 "the JSON responses are parsed with python3"

failures=0
check() {
    local label="$1" found="$2"
    if [ "$found" = "$BARE" ]; then
        ok "$label is at $BARE"
    else
        printf "%b✗ %s is at '%s', expected %s%b\n" "$RED" "$label" "$found" "$BARE" "$RESET"
        failures=$((failures + 1))
    fi
}

step "GitHub Release"
gh_version="$(curl -fsSL "https://api.github.com/repos/Nimblesite/Basilisk/releases/latest" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"].lstrip("v"))' 2>/dev/null || echo "")"
check "GitHub Releases" "$gh_version"

step "PyPI"
pypi_version="$(curl -fsSL "https://pypi.org/pypi/basilisk-python/json" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["info"]["version"])' 2>/dev/null || echo "")"
check "PyPI basilisk-python" "$pypi_version"

step "VS Code Marketplace"
marketplace_version="$(curl -fsSL \
    -H 'Accept: application/json;api-version=7.2-preview.1' \
    -H 'Content-Type: application/json' \
    -X POST 'https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery' \
    -d '{"filters":[{"criteria":[{"filterType":7,"value":"Nimblesite.basilisk"}]}],"flags":914}' |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["results"][0]["extensions"][0]["versions"][0]["version"])' 2>/dev/null || echo "")"
check "VS Code Marketplace" "$marketplace_version"

step "Open VSX"
ovsx_version="$(curl -fsSL "https://open-vsx.org/api/Nimblesite/basilisk" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])' 2>/dev/null || echo "")"
check "Open VSX" "$ovsx_version"

step "Homebrew tap"
brew_version="$(curl -fsSL "https://raw.githubusercontent.com/Nimblesite/homebrew-tap/main/Formula/basilisk.rb" |
    sed -n 's/^  version "\(.*\)"$/\1/p' || echo "")"
check "Homebrew tap" "$brew_version"

step "Scoop bucket"
scoop_version="$(curl -fsSL "https://raw.githubusercontent.com/Nimblesite/scoop-bucket/main/bucket/basilisk.json" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])' 2>/dev/null || echo "")"
check "Scoop bucket" "$scoop_version"

step "Neovim mirror tag"
nvim_tag="$(curl -fsSL "https://api.github.com/repos/Nimblesite/basilisk.nvim/tags" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["name"].lstrip("v"))' 2>/dev/null || echo "")"
check "Nimblesite/basilisk.nvim" "$nvim_tag"

# Zed does not ship from the release workflow — delist/00-publish-zed-final.sh
# pushes the mirror by hand, and the mirror is the whole Zed surface: Basilisk
# is not in the zed-industries registry and never was ([ZED-MIRROR]).
step "Zed mirror tag"
zed_tag="$(curl -fsSL "https://api.github.com/repos/Nimblesite/basilisk-zed/tags" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["name"].lstrip("v"))' 2>/dev/null || echo "")"
check "Nimblesite/basilisk-zed" "$zed_tag"

# Not a version check: an entry appearing here at all would mean Basilisk got
# listed on Zed during its unlisting, and both Zed scripts refuse to run.
step "Zed registry entry"
zed_listed="$(curl -fsSL "https://raw.githubusercontent.com/zed-industries/extensions/main/extensions.toml" |
    python3 -c 'import sys,tomllib; print(tomllib.loads(sys.stdin.read()).get("basilisk", {}).get("version", ""))' 2>/dev/null || echo "")"
if [ -z "$zed_listed" ]; then
    ok "Zed registry lists no basilisk entry — as expected; nothing to unlist there"
else
    warn "Zed registry now lists basilisk at '$zed_listed' — it was never listed before."
    warn "Re-read delist/06-unlist-zed.sh: a removal PR is needed after all."
fi

echo
if [ "$failures" -ne 0 ]; then
    fail "$failures channel(s) are not on $BARE — DO NOT UNLIST YET. Publish the final version first."
fi
ok "every channel is on $BARE — the statement has reached existing installs; unlisting may proceed"
