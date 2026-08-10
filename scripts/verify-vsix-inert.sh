#!/usr/bin/env bash
# Assert that a packaged VSIX contains no type checker.
#
# Implements [WITHDRAWAL-SURFACES]. See
# docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-SURFACES
#
# The extension is a notice. Its package must carry the compiled notice, the
# licence, and nothing else — no `basilisk` binary, no vendored debugger, no
# compiled client for a language server that no longer exists. Packaging is
# governed by .vscodeignore and package.json, both of which a future edit could
# quietly widen, so the shipped zip is inspected rather than trusted.
#
#   scripts/verify-vsix-inert.sh basilisk.vsix

set -euo pipefail

vsix="${1:?usage: verify-vsix-inert.sh <file.vsix>}"
[ -f "$vsix" ] || { echo "::error::no such VSIX: $vsix" >&2; exit 1; }

entries="$(unzip -Z1 "$vsix")"

fail=0
note() { echo "::error::$vsix $1" >&2; fail=1; }

# Any executable or vendored runtime. `bin/` held the per-platform `basilisk`
# binary; `bundled/` held debugpy.
while IFS= read -r entry; do
    case "$entry" in
        extension/bin/*|extension/bundled/*)
            note "ships a runtime artifact: $entry" ;;
        *basilisk-profiler-helper*|*basilisk.exe|extension/basilisk)
            note "ships a binary: $entry" ;;
    esac
done <<< "$entries"

# The compiled client for the withdrawn features. Only the notice and its
# generated copy may be present.
compiled="$(grep -E '^extension/out/.*\.js$' <<< "$entries" || true)"
while IFS= read -r entry; do
    [ -z "$entry" ] && continue
    case "$entry" in
        extension/out/extension.js|extension/out/withdrawal-notice.js) ;;
        *) note "ships a compiled module that is not the notice: $entry" ;;
    esac
done <<< "$compiled"

# The notice itself must be there — an empty package would "pass" every check
# above while telling the user nothing.
grep -qx 'extension/out/extension.js' <<< "$entries" ||
    note "is missing extension/out/extension.js"
# vsce lowercases the readme entry, so match without case.
grep -qix 'extension/readme.md' <<< "$entries" ||
    note "is missing the README"
grep -qx 'extension/LICENSE.txt' <<< "$entries" ||
    note "is missing extension/LICENSE.txt"

if [ "$fail" -ne 0 ]; then
    echo "::error::the VSIX must ship the notice and nothing else" >&2
    exit 1
fi
echo "✓ $vsix ships the notice and no type checker"
