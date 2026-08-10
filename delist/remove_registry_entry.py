#!/usr/bin/env python3
"""Delete one `[name]` block from a Zed registry `extensions.toml`.

Implements [WITHDRAWAL-UNLIST]. The registry file is a flat list of
`[extension-id]` blocks in a repository we do not own, so the edit must be
surgical: remove exactly the named block and leave every other byte — ordering,
spacing, comments — untouched, or the removal PR arrives full of unrelated diff.

    delist/remove_registry_entry.py path/to/extensions.toml basilisk
"""

from __future__ import annotations

import sys
from pathlib import Path


def without_block(text: str, name: str) -> str:
    """`text` with the `[name]` block and its trailing blank line removed."""
    header = f"[{name}]"
    lines = text.splitlines(keepends=True)
    kept: list[str] = []
    dropping = False
    for line in lines:
        if line.strip() == header:
            dropping = True
            continue
        if dropping:
            # The block ends at the next header, or at the blank line before it.
            if line.startswith("["):
                dropping = False
            elif not line.strip():
                dropping = False
                continue
            else:
                continue
        kept.append(line)
    return "".join(kept)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    path, name = Path(argv[1]), argv[2]
    text = path.read_text(encoding="utf-8")
    if f"[{name}]" not in text:
        print(f"{path}: no [{name}] entry — already removed", file=sys.stderr)
        return 0
    path.write_text(without_block(text, name), encoding="utf-8")
    print(f"removed [{name}] from {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
