#!/usr/bin/env bash
# Prove every channel is actually unlisted.
#
# Implements [WITHDRAWAL-UNLIST]. A script that ran without erroring is not
# evidence that a listing is gone — a PAT can be scoped wrong, a PR can sit
# unmerged, a CDN can serve a cached page. This asks each channel's public API
# the same question a user's tooling would, and reports what it actually sees.
#
# Read-only. Run it after the unlisting scripts, and again a day later.
#
#   delist/08-verify-unlisted.sh

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

require_cmd curl "the channel checks are plain HTTP"

still_listed=0

# Report on a URL that MUST NOT resolve to a live listing any more.
gone() {
    local label="$1" url="$2"
    local code
    code="$(curl -o /dev/null -sw '%{http_code}' -L "$url" || echo "000")"
    case "$code" in
        404|410) ok "$label: gone ($code)" ;;
        000)     warn "$label: could not be reached — check by hand: $url" ;;
        *)
            printf "%b✗ %s: STILL LISTED (%s) — %s%b\n" "$RED" "$label" "$code" "$url" "$RESET"
            still_listed=$((still_listed + 1))
            ;;
    esac
}

step "Channels that must 404"
gone "VS Code Marketplace" "https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk"
gone "Open VSX"            "https://open-vsx.org/api/Nimblesite/basilisk"
gone "Homebrew formula"    "https://raw.githubusercontent.com/Nimblesite/homebrew-tap/main/Formula/basilisk.rb"
gone "Scoop manifest"      "https://raw.githubusercontent.com/Nimblesite/scoop-bucket/main/bucket/basilisk.json"
gone "Zed registry entry"  "https://raw.githubusercontent.com/zed-industries/extensions/main/extensions/basilisk/extension.toml"

step "PyPI — yanked, not deleted"
# Yanking keeps the files installable by exact pin (so existing lockfiles do not
# break) while removing the release from resolution. `yanked` is the field pip
# reads, so it is the field that matters here.
yanked="$(curl -fsSL https://pypi.org/pypi/basilisk-python/json |
    python3 -c '
import json, sys
data = json.load(sys.stdin)
releases = data.get("releases", {})
live = [v for v, files in releases.items() if files and not all(f.get("yanked") for f in files)]
print(",".join(sorted(live)) if live else "")
' 2>/dev/null || echo "unreachable")"
if [ -z "$yanked" ]; then
    ok "PyPI: every release is yanked"
elif [ "$yanked" = "unreachable" ]; then
    warn "PyPI: project not found (fully deleted) or unreachable"
else
    printf "%b✗ PyPI: these releases are NOT yanked: %s%b\n" "$RED" "$yanked" "$RESET"
    still_listed=$((still_listed + 1))
fi

step "Surfaces that must STAY up"
for url in \
    "https://www.basilisk-python.dev/" \
    "https://github.com/Nimblesite/Basilisk" \
    "https://api.github.com/repos/Nimblesite/Basilisk/releases"
do
    code="$(curl -o /dev/null -sw '%{http_code}' -L "$url" || echo "000")"
    if [ "$code" = "200" ]; then
        ok "still up: $url"
    else
        printf "%b✗ MISSING (%s): %s — the record must stay public%b\n" "$RED" "$code" "$url" "$RESET"
        still_listed=$((still_listed + 1))
    fi
done

echo
if [ "$still_listed" -ne 0 ]; then
    fail "$still_listed check(s) failed — unlisting is not complete"
fi
ok "every channel is unlisted, and the statement and the record are still public"
