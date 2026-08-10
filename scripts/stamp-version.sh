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

# Files that take the full SemVer including any pre-release suffix
# (e.g. `0.1.0-alpha`). Git tags, Cargo, our own binaries, the shipwright
# contract, and the GitHub release all carry this.
#
# `website/src/_data/site.json` used to be here. It is gone: the site is one
# statement page and displays no version, so its metadata is now derived in
# `site.js` from the generated withdrawal copy. A carrier listed here that does
# not exist is fatal (`stamp_file` exits 2), and this script is the FIRST step
# of the build, release, vsix and pypi-wheels jobs — leaving it listed failed
# every one of them, so no release could ship at all.
readonly FILES=(
    "Cargo.toml"
    "basilisk-zed/Cargo.toml"
    "basilisk-zed/extension.toml"
    "shipwright.json"
)

# Files that take a Marketplace-legal MAJOR.MINOR.PATCH only. The VS
# Marketplace rejects SemVer pre-release suffixes in the extension version
# outright ("We only support major.minor.patch"); pre-release status is
# conveyed via the --pre-release flag to `vsce package`/`vsce publish` in
# release.yml. The unsuffixed core matches the version in every other
# file, so the bundled binary's --version and the manifest stay aligned.
readonly MARKETPLACE_FILES=(
    "vscode-extension/package.json"
)

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

stamp_file() {
    local file="$1" value="$2"
    if [[ ! -f "$file" ]]; then
        echo "stamp-version.sh: missing file: $file" >&2
        exit 2
    fi
    if ! grep -qF "$PLACEHOLDER" "$file"; then
        echo "stamp-version.sh: $file does not contain $PLACEHOLDER (already stamped?)" >&2
        exit 2
    fi
    # Portable in-place edit (BSD + GNU sed both accept this with -i.bak).
    sed -i.bak "s/${PLACEHOLDER}/${value}/g" "$file"
    rm -f "${file}.bak"
    echo "  stamped $file -> $value"
}

main() {
    local version
    version="$(resolve_version "$@")"
    validate_semver "$version"

    # Marketplace version is the core MAJOR.MINOR.PATCH with any pre-release
    # and build-metadata suffixes stripped (0.1.0-alpha.2+sha.abc -> 0.1.0).
    local marketplace_version="${version%%[-+]*}"

    local root
    root="$(repo_root)"
    cd "$root"

    local total=$((${#FILES[@]} + ${#MARKETPLACE_FILES[@]}))
    echo "Stamping $total files (version=$version, marketplace=$marketplace_version)..."

    for file in "${FILES[@]}"; do
        stamp_file "$file" "$version"
    done
    for file in "${MARKETPLACE_FILES[@]}"; do
        stamp_file "$file" "$marketplace_version"
    done

    echo "Done. Verifying no placeholders remain..."
    local leftover
    leftover="$(grep -lF "$PLACEHOLDER" "${FILES[@]}" "${MARKETPLACE_FILES[@]}" 2>/dev/null || true)"
    if [[ -n "$leftover" ]]; then
        echo "stamp-version.sh: placeholders still present in:" >&2
        echo "$leftover" >&2
        exit 2
    fi
    echo "All files stamped."
}

main "$@"
