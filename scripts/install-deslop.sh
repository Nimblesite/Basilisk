#!/usr/bin/env bash
# Install the LATEST `deslop` duplication-gate CLI ([CI-DESLOP]).
#
# deslop is deliberately UNPINNED. The CLI, the MCP server, and the LSP/VSIX
# panel must all run the SAME analysis engine — a stale CLI silently analyses a
# different corpus than the live panel (e.g. an old CLI with no TypeScript
# parser drops the whole VSIX + website codebase from the metric), so the CI
# gate and the editor disagree. Tracking the newest release keeps the CLI in
# lockstep with the engine the editor auto-updates to.
#
# Single source of truth for the platform→asset mapping and the "always latest"
# policy. Shared by scripts/setup.sh (local dev) and .github/workflows/ci.yml so
# CI grabs deslop fresh and installs the latest version whenever it is not
# already present.
#
# Usage:
#   scripts/install-deslop.sh [INSTALL_DIR]
#
# INSTALL_DIR defaults to /usr/local/bin when writable, else ~/.local/bin.
# When run in CI ($GITHUB_PATH set) the chosen dir is appended to $GITHUB_PATH
# so subsequent workflow steps (e.g. `make lint`) can find the binary.
#
# DESLOP_VERSION is an optional escape hatch: set it to a concrete version to
# override the "latest" default (used by tooling that needs a specific build).
# Leave it unset — the default — to always track the latest release.
set -euo pipefail

REPO="Nimblesite/Deslop"
API_LATEST="https://api.github.com/repos/${REPO}/releases/latest"

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
say()  { echo -e "${CYAN}${BOLD}▶ $*${RESET}"; }
ok()   { echo -e "${GREEN}✓ $*${RESET}"; }
fail() { echo -e "${RED}✗ $*${RESET}" >&2; exit 1; }

# Resolve the latest published release tag (e.g. "v0.24.0" → "0.24.0") from the
# GitHub REST API. Authenticates with $GITHUB_TOKEN when present so CI does not
# hit the 60/hour unauthenticated rate limit. JSON is parsed with python3 (a
# hard dependency of this repo's conformance scorer, so always available).
resolve_latest() {
    local headers=(-H "Accept: application/vnd.github+json")
    [[ -n "${GITHUB_TOKEN:-}" ]] && headers+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
    curl -sSfL "${headers[@]}" "$API_LATEST" \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"].lstrip("v"))'
}

# Concrete version wins; otherwise resolve the latest release.
version="${DESLOP_VERSION:-}"
if [[ -z "$version" || "$version" == "latest" ]]; then
    say "Resolving latest deslop release"
    version="$(resolve_latest)" || fail "could not resolve latest deslop release from $API_LATEST"
    [[ -n "$version" ]] || fail "resolved an empty deslop version from $API_LATEST"
    ok "latest deslop release is ${version}"
fi

base_url="https://github.com/${REPO}/releases/download/v${version}"

# Already at the target version? Nothing to do (keeps `make setup`/CI idempotent
# while still guaranteeing the newest release, since `version` is re-resolved
# every run before this check).
if command -v deslop &>/dev/null && deslop --version 2>/dev/null | grep -q -- "${version}"; then
    ok "deslop ${version} already installed ($(command -v deslop))"
    # Still expose the existing binary's dir to later CI steps.
    if [[ -n "${GITHUB_PATH:-}" ]]; then
        dirname "$(command -v deslop)" >> "$GITHUB_PATH"
    fi
    exit 0
fi

case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=macos ;;
    *) fail "Unsupported OS for deslop install: $(uname -s) — install manually: $base_url" ;;
esac
case "$(uname -m)" in
    arm64|aarch64) arch=arm64 ;;
    x86_64|amd64)  arch=x64 ;;
    *) fail "Unsupported arch for deslop install: $(uname -m) — install manually: $base_url" ;;
esac

stem="deslop-${version}-${os}-${arch}"
asset="${stem}.tar.gz"

# Pick an install dir on PATH (explicit arg wins; else writable system bin; else user bin).
dest="${1:-}"
if [[ -z "$dest" ]]; then
    if [[ -w /usr/local/bin ]]; then dest=/usr/local/bin; else dest="$HOME/.local/bin"; fi
fi
mkdir -p "$dest"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading $asset"
curl -sSfL -o "$tmp/$asset" "$base_url/$asset"
curl -sSfL -o "$tmp/$asset.sha256" "$base_url/$asset.sha256"

say "Verifying SHA-256"
expected="$(awk '{print $1}' "$tmp/$asset.sha256")"
if command -v sha256sum &>/dev/null; then
    actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
else
    actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
fi
[[ "$expected" == "$actual" ]] || fail "SHA-256 mismatch: expected $expected, got $actual"
ok "checksum verified"

tar -xzf "$tmp/$asset" -C "$tmp"
install -m 0755 "$tmp/$stem/deslop" "$dest/deslop"
ok "installed deslop ${version} → $dest/deslop"

# CI: make the install dir discoverable by subsequent steps.
if [[ -n "${GITHUB_PATH:-}" ]]; then
    echo "$dest" >> "$GITHUB_PATH"
fi

# Local: nudge if the dir isn't already on PATH.
case ":$PATH:" in
    *":$dest:"*) ;;
    *) echo -e "${CYAN}  Note: add $dest to your PATH to run 'deslop' directly.${RESET}" ;;
esac
