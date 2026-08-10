#!/usr/bin/env python3
"""Generate the body of the final release.

Implements [WITHDRAWAL-SURFACES]. A GitHub Release is a public surface, and this
one is the last: it carries the inert CLI to every installed copy. Auto-generated
"what's changed" notes would list commits under a heading that reads like a
product update, so the body is the approved statement instead — copied from
docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md, never written here.

    python3 scripts/gen_release_notes.py v0.42.0 > release-notes.md
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_withdrawal_copy import copy_blocks  # noqa: E402


def notes(version: str) -> str:
    """The release body: the statement, then what this build does."""
    copy = copy_blocks()
    body = [f"# {copy.title}", ""]
    for block in (copy.short, copy.action):
        for paragraph in block:
            body += [paragraph, ""]
    body += [
        "## This release",
        "",
        f"`{version}` is the final Basilisk release. It exists to deliver the "
        "statement above to installations that already exist:",
        "",
        "- The `basilisk` CLI is inert. Every invocation prints the statement to "
        "stderr and exits `4`. It reads no file, starts no server, and checks nothing.",
        "- The VS Code extension bundles no checker. It shows the statement and "
        "contributes nothing else.",
        "- The Neovim plugin starts no language server. It shows the statement.",
        "",
        "Every distribution channel is unlisted immediately after this release. "
        "Earlier releases stay published: deleting them would destroy the record.",
        "",
    ]
    return "\n".join(body)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    print(notes(argv[1]), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
