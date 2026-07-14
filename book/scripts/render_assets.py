#!/usr/bin/env python3
"""Render deterministic SVG masters into book publication PNGs."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from pathlib import Path


BOOK_ROOT = Path(__file__).resolve().parents[1]


def require_tool(name: str) -> str:
    """Return an executable path or fail with a useful message."""
    executable = shutil.which(name)
    if executable is None:
        raise SystemExit(f"Required rendering tool is missing: {name}")
    return executable


def run(command: list[str]) -> None:
    """Run one renderer command from the book root."""
    subprocess.run(command, cwd=BOOK_ROOT, check=True)


def render_svg(rsvg: str, source: Path, target: Path, canvas: str) -> None:
    """Render one SVG at its declared canvas size."""
    width, height = canvas.split("x", maxsplit=1)
    target.parent.mkdir(parents=True, exist_ok=True)
    run(
        [
            rsvg,
            "--width",
            width,
            "--height",
            height,
            str(source),
            "--output",
            str(target),
        ]
    )


def render_cover(rsvg: str, magick: str, manifest: dict[str, object]) -> None:
    """Render the cover base and composite the canonical raster brand mark."""
    cover = manifest["cover"]
    if not isinstance(cover, dict):
        raise SystemExit("figures.json cover entry must be an object")

    source = BOOK_ROOT / str(cover["master"])
    target = BOOK_ROOT / str(cover["path"])
    logo = BOOK_ROOT / "assets/brand/basilisk-logo.png"
    if not source.is_file() or not logo.is_file():
        raise SystemExit("Cover source or Basilisk brand mark is missing")

    target.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="basilisk-book-cover-") as directory:
        base = Path(directory) / "cover-base.png"
        render_svg(rsvg, source, base, "1600x2560")
        run(
            [
                magick,
                str(base),
                "(",
                str(logo),
                "-resize",
                "336x336",
                ")",
                "-geometry",
                "+632+242",
                "-composite",
                "-strip",
                str(target),
            ]
        )


def main() -> None:
    """Render the cover and every ready SVG figure."""
    rsvg = require_tool("rsvg-convert")
    magick = require_tool("magick")
    manifest = json.loads((BOOK_ROOT / "figures.json").read_text(encoding="utf-8"))
    targets = manifest["targets"]
    if not isinstance(targets, dict):
        raise SystemExit("figures.json targets entry must be an object")

    render_cover(rsvg, magick, manifest)
    canvas = str(targets["diagramCanvas"])
    for figure in manifest["figures"]:
        if figure.get("status") != "ready" or "master" not in figure:
            continue
        source = BOOK_ROOT / str(figure["master"])
        target = BOOK_ROOT / str(figure["path"])
        if source.suffix.lower() != ".svg":
            continue
        render_svg(rsvg, source, target, canvas)

    print("Rendered ready book assets.")


if __name__ == "__main__":
    main()
