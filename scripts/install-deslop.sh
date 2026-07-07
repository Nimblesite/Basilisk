#!/usr/bin/env bash
# Install the deslop duplication-gate CLI ([CI-DESLOP]) via Homebrew.
#
# Unpinned on purpose: the nimblesite/tap formula tracks the latest release, so
# the CLI stays in lockstep with the engine the editor's LSP/MCP panel runs. A
# stale CLI analyses a different corpus than the panel and the gate disagrees
# with the editor. Shared by scripts/setup.sh (local) and ci.yml (ubuntu-24.04,
# which ships Homebrew) so both grab deslop fresh at its latest release.
set -euo pipefail

command -v brew >/dev/null || {
    echo "deslop needs Homebrew — https://brew.sh (or grab a binary: https://github.com/Nimblesite/Deslop/releases)" >&2
    exit 1
}

brew install nimblesite/tap/deslop || brew upgrade nimblesite/tap/deslop

# Make the brew bin discoverable to later CI steps.
if [[ -n "${GITHUB_PATH:-}" ]]; then
    echo "$(brew --prefix)/bin" >> "$GITHUB_PATH"
fi
