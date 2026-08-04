#!/usr/bin/env bash
# Install the deslop duplication-gate CLI ([CI-DESLOP]) via Homebrew, falling
# back to the release tarball the formula itself ships when brew is absent.
#
# Unpinned on purpose: the nimblesite/tap formula tracks the latest release, so
# the CLI stays in lockstep with the engine the editor's LSP/MCP panel runs. A
# stale CLI analyses a different corpus than the panel and the gate disagrees
# with the editor. Shared by scripts/setup.sh (local) and ci.yml (ubuntu-24.04,
# which ships Homebrew) so both grab deslop fresh at its latest release.
set -euo pipefail

# Where the brew-less fallback puts the binaries. ~/.local/bin is on PATH by
# default under fish and most distro profiles; override for anywhere else.
DESLOP_BIN_DIR="${DESLOP_BIN_DIR:-$HOME/.local/bin}"

# CI runs steps with `bash --noprofile --norc`, which skips the profile scripts
# that put Homebrew on PATH. Load `brew` from its known install locations —
# /home/linuxbrew on GitHub's ubuntu runners, /opt/homebrew (Apple Silicon) or
# /usr/local (Intel) on macOS — so `brew` resolves in CI and local shells alike.
if ! command -v brew >/dev/null; then
    for brew_bin in /home/linuxbrew/.linuxbrew/bin/brew /opt/homebrew/bin/brew /usr/local/bin/brew; do
        if [[ -x "$brew_bin" ]]; then
            eval "$("$brew_bin" shellenv)"
            break
        fi
    done
fi

# Resolve this machine's release asset suffix (`-linux-x64.tar.gz` &c). Echoes
# the suffix; fails on a platform the project publishes no binary for.
deslop_asset_suffix() {
    local os arch
    case "$(uname -s)" in
        Darwin) os=macos ;;
        Linux) os=linux ;;
        *) echo "deslop: no published binary for $(uname -s)" >&2; return 1 ;;
    esac
    case "$(uname -m)" in
        x86_64 | amd64) arch=x64 ;;
        arm64 | aarch64) arch=arm64 ;;
        *) echo "deslop: no published binary for $(uname -m)" >&2; return 1 ;;
    esac
    echo "-$os-$arch.tar.gz"
}

# Install deslop from its latest GitHub release for machines without Homebrew —
# most Linux distros, and any workstation whose user cannot sudo. The tap's
# formula is a binary formula over these very tarballs, so this lands the
# identical CLI at the identical version; the checksum is verified exactly as
# brew verifies the formula's `sha256`.
install_deslop_from_release() {
    local suffix release tag tarball_url tmp asset
    suffix="$(deslop_asset_suffix)" || return 1
    command -v python3 >/dev/null || {
        echo "deslop: the Homebrew-free install needs python3 to read the release manifest" >&2
        return 1
    }

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN

    release="$tmp/release.json"
    curl -fsSL https://api.github.com/repos/Nimblesite/Deslop/releases/latest -o "$release"
    read -r tag tarball_url < <(python3 - "$release" "$suffix" <<'PY'
import json, sys

with open(sys.argv[1], encoding="utf-8") as handle:
    release = json.load(handle)
assets = [a for a in release["assets"] if a["name"].endswith(sys.argv[2])]
if not assets:
    sys.exit(f"deslop: no release asset ending in {sys.argv[2]}")
print(release["tag_name"], assets[0]["browser_download_url"])
PY
    )

    asset="$(basename "$tarball_url")"
    echo "deslop: Homebrew unavailable — installing $tag from $asset" >&2
    curl -fsSL "$tarball_url" -o "$tmp/$asset"
    curl -fsSL "$tarball_url.sha256" -o "$tmp/$asset.sha256"
    if command -v sha256sum >/dev/null; then
        (cd "$tmp" && sha256sum -c "$asset.sha256")
    else
        (cd "$tmp" && shasum -a 256 -c "$asset.sha256")
    fi

    tar xzf "$tmp/$asset" -C "$tmp"
    mkdir -p "$DESLOP_BIN_DIR"
    # Install all three unconditionally, matching the formula: a tarball missing
    # one must fail loudly rather than leave deslop-mcp off PATH (Deslop #240).
    local binary
    for binary in deslop deslop-lsp deslop-mcp; do
        install -m 755 "$tmp/${asset%.tar.gz}/$binary" "$DESLOP_BIN_DIR/$binary"
    done

    case ":$PATH:" in
        *":$DESLOP_BIN_DIR:"*) ;;
        *) echo "deslop: add $DESLOP_BIN_DIR to PATH to reach the CLI" >&2 ;;
    esac
}

if ! command -v brew >/dev/null; then
    install_deslop_from_release
    if [[ -n "${GITHUB_PATH:-}" ]]; then
        echo "$DESLOP_BIN_DIR" >> "$GITHUB_PATH"
    fi
    exit 0
fi

# Every `brew` invocation may trigger an auto-update that talks to the network,
# and those fail intermittently on CI runners — one Lint run died with a bare
# "Broken pipe" partway through a step that had succeeded minutes earlier. Retry
# each brew call a few times before giving up, so a blip costs seconds rather
# than a whole pipeline.
readonly BREW_ATTEMPTS=3
readonly BREW_BACKOFF_SECONDS=5

# Run `brew "$@"`, retrying transient failures. Returns brew's last exit status.
brew_retry() {
    local attempt=1
    until brew "$@"; do
        local status=$?
        if ((attempt >= BREW_ATTEMPTS)); then
            return "$status"
        fi
        echo "brew $* failed (attempt $attempt/$BREW_ATTEMPTS); retrying" >&2
        attempt=$((attempt + 1))
        sleep "$BREW_BACKOFF_SECONDS"
    done
}

# Homebrew >= 6 gates formulae from non-official taps behind
# HOMEBREW_REQUIRE_TAP_TRUST (set by default on GitHub's ubuntu runners). Under
# that gate an untrusted-tap install prompts for confirmation; in CI stdin is
# closed, so the prompt dies as a "Broken pipe" and the install aborts. Tap
# explicitly, then trust the tap non-interactively so `brew install` proceeds.
brew_retry tap nimblesite/tap

# Best-effort: older Homebrew has no `brew trust` subcommand, and machines
# without the gate never needed one. A genuine trust failure is not swallowed —
# it resurfaces immediately as the install below prompting and aborting.
brew_retry trust --tap nimblesite/tap || echo "brew trust unavailable; installing untrusted" >&2

brew_retry install nimblesite/tap/deslop || brew_retry upgrade nimblesite/tap/deslop

# Make the brew bin discoverable to later CI steps.
if [[ -n "${GITHUB_PATH:-}" ]]; then
    echo "$(brew --prefix)/bin" >> "$GITHUB_PATH"
fi
