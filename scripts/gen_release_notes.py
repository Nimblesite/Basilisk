#!/usr/bin/env python3
"""Generate the release-notes component block. Implements [LSPFMT-RELEASE-NOTES].

Usage: gen_release_notes.py BASILISK_BINARY RELEASE_VERSION [MANIFEST]

Enumerates every shipwright.json component plus the embedded Ruff formatter
version, read from the actual release binary — generated, never hand-typed,
so the notes cannot claim different formatter bytes from the build
(docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-RELEASE-NOTES).
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


def embedded_ruff_version(binary: str) -> str:
    """The `Ruff formatter: X` version the binary itself reports."""
    out = subprocess.run(
        [binary, "--version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=60,
    ).stdout
    match = re.search(r"^Ruff formatter: (\S+)$", out, re.MULTILINE)
    if match is None:
        msg = "binary --version did not report an embedded Ruff formatter line"
        raise RuntimeError(msg)
    return match.group(1)


def component_rows(manifest: dict, release_version: str) -> list[str]:
    """One table row per shipwright.json component."""
    rows: list[str] = []
    for component in manifest["components"]:
        declared = component.get("expectedVersion", "")
        version = (
            release_version if declared == "${PRODUCT_VERSION}" else (declared or "—")
        )
        rows.append(f"| `{component['id']}` | {component['kind']} | {version} |")
    return rows


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    binary, release_version = argv[1], argv[2]
    manifest_path = (
        Path(argv[3])
        if len(argv) > 3
        else Path(__file__).resolve().parent.parent / "shipwright.json"
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    lines = [
        "## Components",
        "",
        "| Component | Kind | Version |",
        "|---|---|---|",
        *component_rows(manifest, release_version),
        "",
        f"Embedded Ruff formatter: `{embedded_ruff_version(binary)}`",
    ]
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
