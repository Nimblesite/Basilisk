#!/usr/bin/env bash
# Install the deslop duplication-gate CLI ([CI-DESLOP]) via Homebrew.
#
# Unpinned on purpose: the nimblesite/tap formula tracks the latest release, so
# the CLI stays in lockstep with the engine the editor's LSP/MCP panel runs. A
# stale CLI analyses a different corpus than the panel and the gate disagrees
# with the editor. Shared by scripts/setup.sh (local) and ci.yml (ubuntu-24.04,
# which ships Homebrew) so both grab deslop fresh at its latest release.
set -euo pipefail

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

command -v brew >/dev/null || {
    echo "deslop needs Homebrew — https://brew.sh (or grab a binary: https://github.com/Nimblesite/Deslop/releases)" >&2
    exit 1
}

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
