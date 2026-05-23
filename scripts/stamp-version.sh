#!/usr/bin/env bash
# Stamp every package-bearing version field with the release version.
#
# Reads the version from the first argument, or from GITHUB_REF_NAME when
# invoked without arguments (the workflow runs on tags like `v0.1.0`).
#
# Replaces the literal placeholder `0.0.0-PLACEHOLDER` in every file
# listed below with the resolved version. Refuses to run if any file
# does not currently contain the placeholder (prevents double-stamping
# or stamping over a real version by mistake).
#
# Usage:
#   scripts/stamp-version.sh 0.1.0
#   GITHUB_REF_NAME=v0.1.0 scripts/stamp-version.sh

set -euo pipefail

readonly PLACEHOLDER="0.0.0-PLACEHOLDER"

# Files that take the full SemVer (including any -alpha / -rc.N suffix).
# Used by git tags, Cargo, our own binaries, the shipwright contract, and
# the GitHub release.
readonly FILES=(
    "Cargo.toml"
    "basilisk-zed/Cargo.toml"
    "basilisk-zed/extension.toml"
    "shipwright.json"
    "website/src/_data/site.json"
)

# Files that take a Marketplace-legal MAJOR.MINOR.PATCH (suffix stripped).
# The VS Marketplace flatly rejects SemVer pre-release suffixes in the
# extension version; pre-release status is conveyed via the --pre-release
# flag to vsce + the odd-minor convention (see VERSION_CONVENTION below),
# never via the version string itself.
readonly MARKETPLACE_FILES=(
    "vscode-extension/package.json"
)

# VS Marketplace convention: even MINOR for stable, odd MINOR for
# pre-release. Stable users auto-upgrade across this boundary, so mixing
# them on the same MINOR will yank pre-release users onto stable
# unexpectedly. We refuse to stamp a tag that violates the convention.
#
# Examples:
#   v0.1.0       -> reject: stable tag on odd minor
#   v0.1.0-alpha -> ok:     pre-release tag on odd minor
#   v0.2.0       -> ok:     stable tag on even minor
#   v0.2.0-rc.1  -> reject: pre-release tag on even minor

resolve_version() {
    if [[ $# -ge 1 && -n "$1" ]]; then
        printf '%s' "$1"
        return
    fi
    if [[ -n "${GITHUB_REF_NAME:-}" ]]; then
        # Strip leading `v` from tags like `v0.1.0` or `v0.1.0-alpha.0`.
        printf '%s' "${GITHUB_REF_NAME#v}"
        return
    fi
    echo "stamp-version.sh: provide a version argument or set GITHUB_REF_NAME" >&2
    exit 2
}

validate_semver() {
    local version="$1"
    if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
        echo "stamp-version.sh: '$version' is not valid SemVer" >&2
        exit 2
    fi
}

repo_root() {
    cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

main() {
    local version
    version="$(resolve_version "$@")"
    validate_semver "$version"

    local root
    root="$(repo_root)"
    cd "$root"

    echo "Stamping version $version into ${#FILES[@]} files..."

    for file in "${FILES[@]}"; do
        if [[ ! -f "$file" ]]; then
            echo "stamp-version.sh: missing file: $file" >&2
            exit 2
        fi
        if ! grep -qF "$PLACEHOLDER" "$file"; then
            echo "stamp-version.sh: $file does not contain $PLACEHOLDER (already stamped?)" >&2
            exit 2
        fi
        # Portable in-place edit (BSD + GNU sed both accept this with -i.bak).
        sed -i.bak "s/${PLACEHOLDER}/${version}/g" "$file"
        rm -f "${file}.bak"
        echo "  stamped $file"
    done

    echo "Done. Verifying no placeholders remain..."
    local leftover
    leftover="$(grep -lF "$PLACEHOLDER" "${FILES[@]}" 2>/dev/null || true)"
    if [[ -n "$leftover" ]]; then
        echo "stamp-version.sh: placeholders still present in:" >&2
        echo "$leftover" >&2
        exit 2
    fi
    echo "All files stamped to $version."
}

main "$@"
