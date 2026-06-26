#!/usr/bin/env bash
# Render a self-contained, version-stamped Zed extension tree for the
# Nimblesite/basilisk-zed mirror. Implements [ZED-DIST] / [ZED-MIRROR];
# see docs/specs/ZED-SPEC.md#ZED-MIRROR.
#
# Why a render step exists. The in-repo basilisk-zed/ crate (a) carries the
# 0.0.0-PLACEHOLDER version that is stamped only during CI, and (b) depends on
# basilisk-common via a *workspace* path (`../crates/basilisk-common`). The Zed
# extension registry (zed-industries/extensions) pins a commit and compiles the
# extension to WASM *standalone*, with no monorepo around it, so it can neither
# pin `main` (placeholder version) nor build it (unresolvable path dep). This
# script produces a tree that the registry can build on its own:
#
#   * vendors basilisk-common (zero-dep, WASM-safe) under vendor/basilisk-common
#   * rewrites the extension manifest's path dep to the vendored copy
#   * makes the mirror dir its own workspace root (empty [workspace] table) so
#     cargo does not search upward for a parent workspace
#   * stamps the release version into Cargo.toml + extension.toml + the
#     vendored crate
#   * drops workspace-only [lints] inheritance (no parent workspace to inherit
#     from in the mirror — lint strictness is enforced by the monorepo `zed` CI
#     job, not by the distribution render)
#   * omits committed build artifacts (extension.wasm, dist/, stale Cargo.lock);
#     the publish job regenerates Cargo.lock via the standalone WASM build gate
#
# Usage:
#   scripts/render-zed-mirror.sh <dest-dir> [version]
#   GITHUB_REF_NAME=v0.1.0 scripts/render-zed-mirror.sh out/

set -euo pipefail

readonly PLACEHOLDER="0.0.0-PLACEHOLDER"

# Curated set copied verbatim from basilisk-zed/ into the mirror root. Anything
# not listed here (extension.wasm, dist/, Cargo.lock, tests/) is intentionally
# excluded — the registry needs only the manifest, sources, and language assets.
readonly COPY_ITEMS=(
    "extension.toml"
    "Cargo.toml"
    "README.md"
    "LICENSE"
    "src"
    "languages"
    "themes"
    "debug_adapter_schemas"
    "images"
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

# Read a `key = "value"` field from the [workspace.package] table of the root
# Cargo.toml so the vendored crate's concrete metadata stays in lockstep with
# the workspace rather than hardcoding duplicate values here.
ws_package_field() {
    local key="$1" file="$2"
    awk -v key="$key" '
        /^\[workspace\.package\]/ { in_section = 1; next }
        /^\[/ { in_section = 0 }
        in_section && $1 == key {
            # Strip everything up to the first quote and the trailing quote.
            line = $0
            sub(/^[^"]*"/, "", line)
            sub(/".*$/, "", line)
            print line
            exit
        }
    ' "$file"
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
            # -L dereferences symlinks so the mirror is self-contained: e.g.
            # images/zed-screenshot.png links into website/, which does not exist
            # in the standalone tree — copy the real file, not a dangling link.
            cp -RL "$src/$item" "$dest/$item"
        fi
    done
}

# Vendor basilisk-common as a standalone crate: concrete metadata (from the
# workspace package table) replaces every `*.workspace = true` inheritance, and
# the [lints] table is dropped.
vendor_common() {
    local root="$1" dest="$2" version="$3"
    local out="$dest/vendor/basilisk-common"
    mkdir -p "$dest/vendor"
    cp -R "$root/crates/basilisk-common" "$out"
    rm -rf "$out/target"

    local edition repository rust_version license
    edition="$(ws_package_field "edition" "$root/Cargo.toml")"
    repository="$(ws_package_field "repository" "$root/Cargo.toml")"
    rust_version="$(ws_package_field "rust-version" "$root/Cargo.toml")"
    license="$(ws_package_field "license" "$root/Cargo.toml")"

    # Resolve the non-version workspace-inherited metadata to concrete literals.
    # The version is stamped structurally below (not via sed) per §3.3.
    local manifest="$out/Cargo.toml"
    sed -i.bak \
        -e "s|^edition\.workspace = true|edition = \"${edition}\"|" \
        -e "s|^license\.workspace = true|license = \"${license}\"|" \
        -e "s|^repository\.workspace = true|repository = \"${repository}\"|" \
        -e "s|^rust-version\.workspace = true|rust-version = \"${rust_version}\"|" \
        "$manifest"
    rm -f "${manifest}.bak"
    stamp_toml_version "$manifest" "$version"
    strip_lints_table < "$manifest" > "${manifest}.stripped"
    mv "${manifest}.stripped" "$manifest"
}

# Rewrite the extension manifest: vendored path dep, no workspace lints, an
# explicit empty [workspace] so the mirror dir is its own root, stamped version.
render_extension_manifest() {
    local dest="$1" version="$2"
    local manifest="$dest/Cargo.toml"
    sed -i.bak \
        -e 's|path = "../crates/basilisk-common"|path = "vendor/basilisk-common"|' \
        "$manifest"
    rm -f "${manifest}.bak"
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
    vendor_common "$root" "$dest" "$version"
    render_extension_manifest "$dest" "$version"
    stamp_toml_version "$dest/extension.toml" "$version"

    echo "Verifying no placeholders remain..."
    if grep -rlF "$PLACEHOLDER" "$dest" >/dev/null 2>&1; then
        echo "render-zed-mirror.sh: placeholders still present under ${dest}:" >&2
        grep -rlF "$PLACEHOLDER" "$dest" >&2
        exit 2
    fi
    echo "Rendered standalone Zed extension at ${dest}"
}

main "$@"
