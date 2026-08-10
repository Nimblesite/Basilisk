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

echo
if [ "$failures" -ne 0 ]; then
    fail "$failures channel(s) are not on $BARE — DO NOT UNLIST YET. Publish the final version first."
fi
ok "every channel is on $BARE — the statement has reached existing installs; unlisting may proceed"
