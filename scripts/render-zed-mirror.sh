#!/usr/bin/env bash
# Render a self-contained, version-stamped Zed extension tree for the
# Nimblesite/basilisk-zed mirror. Implements [ZED-MIRROR];
# see docs/specs/ZED-SPEC.md#ZED-MIRROR.
#
# Why a render step exists. The in-repo basilisk-zed/ crate carries the
# 0.0.0-PLACEHOLDER version that is stamped only during CI, and inherits its
# [lints] from the workspace. The Zed extension registry
# (zed-industries/extensions) pins a commit and compiles the extension to WASM
# *standalone*, with no monorepo around it, so it can neither pin `main`
# (placeholder version) nor resolve workspace inheritance. This script produces
# a tree that the registry can build on its own:
#
#   * makes the mirror dir its own workspace root (empty [workspace] table) so
#     cargo does not search upward for a parent workspace
#   * stamps the release version into Cargo.toml + extension.toml
#   * drops workspace-only [lints] inheritance (no parent workspace to inherit
#     from in the mirror — lint strictness is enforced by the monorepo `zed` CI
#     job, not by the distribution render)
#   * omits committed build artifacts (extension.wasm, dist/, stale Cargo.lock);
#     the publish job regenerates Cargo.lock via the standalone WASM build gate
#
# There is no vendoring step any more: the extension states that Basilisk is
# unlisted and does nothing else, so `zed_extension_api` is its only dependency.
#
# Usage:
#   scripts/render-zed-mirror.sh <dest-dir> [version]
#   GITHUB_REF_NAME=v0.1.0 scripts/render-zed-mirror.sh out/

set -euo pipefail

readonly PLACEHOLDER="0.0.0-PLACEHOLDER"

# Curated set copied verbatim from basilisk-zed/ into the mirror root. Anything
# not listed here (extension.wasm, dist/, stale Cargo.lock) is intentionally
# excluded — the registry needs only the manifest and the sources.
readonly COPY_ITEMS=(
    "extension.toml"
    "Cargo.toml"
    "README.md"
    "LICENSE"
    "src"
)

resolve_version() {
    if [[ $# -ge 1 && -n "$1" ]]; then
        printf '%s' "$1"
        return
    fi
    if [[ -n "${GITHUB_REF_NAME:-}" ]]; then
        printf '%s' "${GITHUB_REF_NAME#v}"
        return
    fi
    echo "render-zed-mirror.sh: provide a version argument or set GITHUB_REF_NAME" >&2
    exit 2
}

validate_semver() {
    local version="$1"
    if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
        echo "render-zed-mirror.sh: '$version' is not valid SemVer" >&2
        exit 2
    fi
}

repo_root() {
    cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

# Drop a whole [lints] table from a Cargo.toml emitted on stdout. The mirror has
# no parent workspace, so `[lints]\nworkspace = true` would fail to resolve.
strip_lints_table() {
    awk '
        /^\[lints(\.|\])/ { skip = 1; next }
        skip && /^\[/     { skip = 0 }
        skip && /^[[:space:]]*$/ { next }
        skip { next }
        { print }
    '
}

# Stamp a TOML carrier's package version structurally (tomllib-validated write,
# not blind sed) via the shared helper. [SWR-VERSION-BUILD-STAMPING] §3.3.
stamp_toml_version() {
    local file="$1" value="$2"
    local here
    here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if command -v python3 >/dev/null 2>&1; then
        python3 "${here}/set_toml_version.py" "$file" "$value"
    else
        python "${here}/set_toml_version.py" "$file" "$value"
    fi
}

# Replace the mirror's tracked content with the freshly rendered tree while
# preserving its own .git directory (the publish job clones the mirror first).
clear_dest() {
    local dest="$1"
    mkdir -p "$dest"
    find "$dest" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
}

copy_extension_tree() {
    local src="$1" dest="$2"
    local item
    for item in "${COPY_ITEMS[@]}"; do
        if [[ -e "$src/$item" ]]; then
            cp -RL "$src/$item" "$dest/$item"
        fi
    done
}

# Rewrite the extension manifest: no workspace lints, an explicit empty
# [workspace] so the mirror dir is its own root, stamped version.
render_extension_manifest() {
    local dest="$1" version="$2"
    local manifest="$dest/Cargo.toml"
    strip_lints_table < "$manifest" > "${manifest}.stripped"
    mv "${manifest}.stripped" "$manifest"
    stamp_toml_version "$manifest" "$version"
    {
        printf '\n# The mirror is its own workspace root so cargo does not search\n'
        printf '# upward for a parent workspace when the registry builds it standalone.\n'
        printf '[workspace]\n'
    } >> "$manifest"
}

main() {
    if [[ $# -lt 1 || -z "$1" ]]; then
        echo "Usage: render-zed-mirror.sh <dest-dir> [version]" >&2
        exit 2
    fi
    local dest="$1"
    shift
    local version
    version="$(resolve_version "$@")"
    validate_semver "$version"

    local root
    root="$(repo_root)"

    echo "Rendering Zed mirror (version=${version}) -> ${dest}"
    clear_dest "$dest"
    copy_extension_tree "$root/basilisk-zed" "$dest"
    render_extension_manifest "$dest" "$version"
    stamp_toml_version "$dest/extension.toml" "$version"

    # Guard against an UNSTAMPED version slipping through. A leftover placeholder
    # only matters as a *version value*, which is always a quoted TOML string
    # (`version = "0.0.0-PLACEHOLDER"`); a doc comment that merely names the token
    # is not a defect. Match the quoted form so prose can reference the
    # placeholder freely without tripping the release ([ZED-MIRROR]).
    echo "Verifying no version placeholders remain..."
    if grep -rlF "\"${PLACEHOLDER}\"" "$dest" >/dev/null 2>&1; then
        echo "render-zed-mirror.sh: unstamped version placeholder under ${dest}:" >&2
        grep -rlF "\"${PLACEHOLDER}\"" "$dest" >&2
        exit 2
    fi
    echo "Rendered standalone Zed extension at ${dest}"
}

main "$@"
