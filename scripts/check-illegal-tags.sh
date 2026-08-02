#!/usr/bin/env bash
# Implements [CHKARCH-SAFETY]: some words are banned as IDENTIFIERS in this
# repository even though they are fine as prose.
#
# `mojo` is the standing case. Basilisk's ownership analysis takes its concepts
# from Mojo's ownership model, but the rules are Basilisk's own — so a doc may
# credit the inspiration in a sentence (with a link), while no spec ID, module
# path, file name, or rule name may carry the name. "Reviewers will notice" has
# already failed repeatedly; the build notices instead.
#
# Banned as a tag:   [CHKARCH-MOJO-SAFETY]  {#chkadvplan-todo-mojo}
#                    mojo_safety.rs  crates/basilisk-mojo/  mojo::check
# Allowed as prose:  "borrowed from Mojo's ownership model"
#                    https://docs.modular.com/mojo/manual/values/ownership
set -euo pipefail
cd "$(dirname "$0")/.."

# Words that may never appear in an identifier. Add to this list, never remove.
ILLEGAL_TAGS=(mojo)

# This script necessarily spells the banned words, so it excludes itself.
SELF=':(exclude)scripts/check-illegal-tags.sh'

fail() {
  echo "ILLEGAL TAG: $1" >&2
  echo "  Prose may credit the inspiration; tags may not carry it. See [CHKARCH-SAFETY]." >&2
  exit 1
}

for tag in "${ILLEGAL_TAGS[@]}"; do
  # A tracked path naming the tag — file or directory.
  if git ls-files | grep -i -- "$tag" >&2; then
    fail "the paths above name '$tag'; rename them after what the code does"
  fi

  # A spec ID or anchor carrying the tag: [GROUP-TAG-DETAIL] or {#group-tag}.
  if git grep -nIiE "(\[|\{#)[A-Za-z0-9-]*${tag}[A-Za-z0-9-]*(\]|\})" -- . "$SELF" >&2; then
    fail "the spec IDs above carry '$tag'; name the section after the analysis"
  fi

  # A snake_case / path identifier: mojo_safety, check_mojo, mojo::check.
  if git grep -nIiE "(${tag}_|_${tag}|${tag}::)" -- . "$SELF" >&2; then
    fail "the identifiers above carry '$tag'; name them after what they do"
  fi
done

echo "illegal-tag check OK: no banned word appears as an identifier"
