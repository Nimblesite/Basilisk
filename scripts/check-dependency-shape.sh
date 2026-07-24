#!/usr/bin/env bash
# Implements [TYPESHEDRT-SEGREGATION]: "the checker never downloads" is a
# property of the BUILD, not a code-review convention. The analysis crates
# (basilisk-stubs, basilisk-checker) must resolve to a dependency graph that
# contains no HTTP client and no basilisk-typeshed-fetch — downloading exists
# only behind explicit user actions (`basilisk typeshed download`, the
# editor's Download buttons).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() {
  echo "DEPENDENCY SHAPE VIOLATION: $1" >&2
  exit 1
}

# Match crate names at line start in `cargo tree --prefix none` output.
forbid_in_graph() {
  local package="$1" pattern="$2" reason="$3"
  if cargo tree -p "$package" -e normal --prefix none | grep -Eq "$pattern"; then
    fail "$reason (matched '$pattern' in ${package}'s resolved graph)"
  fi
}

for analysis_crate in basilisk-stubs basilisk-checker; do
  forbid_in_graph "$analysis_crate" \
    '^(ureq|reqwest|hyper|curl|isahc|attohttpc) ' \
    "$analysis_crate must carry no HTTP client — the analysis path is offline by construction [STUBRES-TYPESHED-OFFLINE]"
  forbid_in_graph "$analysis_crate" \
    '^basilisk-typeshed-fetch ' \
    "$analysis_crate must not link the download component [TYPESHEDRT-SEGREGATION]"
done

# The download component is the ONE place an HTTP client is allowed; make its
# presence explicit so a refactor that silently drops the client (leaving a
# download command that can never work) is caught too.
if ! cargo tree -p basilisk-typeshed-fetch -e normal --prefix none | grep -Eq '^ureq '; then
  fail "basilisk-typeshed-fetch must carry its own HTTPS client [STUBRES-TYPESHED-DOWNLOAD]"
fi

echo "dependency shape OK: analysis crates are offline; downloads live only in basilisk-typeshed-fetch"
