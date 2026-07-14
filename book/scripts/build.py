#!/usr/bin/env python3
"""Build and validate The Basilisk Book as EPUB3."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path


BOOK_ROOT = Path(__file__).resolve().parents[1]


def require_tool(name: str) -> str:
    """Return an executable path or fail with a clear build dependency error."""
    executable = shutil.which(name)
    if executable is None:
        raise SystemExit(f"Required book tool is missing: {name}")
    return executable


def run(command: list[str]) -> None:
    """Run a checked command in the book root."""
    subprocess.run(command, cwd=BOOK_ROOT, check=True)


def main() -> None:
    """Run checks, render assets, build EPUB3, and run EPUBCheck."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--release", action="store_true", help="enforce publication gates"
    )
    args = parser.parse_args()

    python = require_tool("python3")
    pandoc = require_tool("pandoc")
    epubcheck = require_tool("epubcheck")
    manifest = json.loads((BOOK_ROOT / "book.json").read_text(encoding="utf-8"))
    files = [str(section["file"]) for section in manifest["sections"]]

    check_book = [python, "scripts/check_book.py"]
    if args.release:
        check_book.append("--release")
    run(check_book)

    check_links = [python, "scripts/check_links.py"]
    if args.release:
        check_links.append("--external")
    run(check_links)
    run([python, "scripts/render_assets.py"])

    output_name = (
        "the-basilisk-book.epub" if args.release else "the-basilisk-book-outline.epub"
    )
    output = BOOK_ROOT / "dist" / output_name
    output.parent.mkdir(parents=True, exist_ok=True)
    run(
        [
            pandoc,
            *files,
            "--from=gfm",
            "--to=epub3",
            f"--output={output}",
            "--metadata-file=metadata.yaml",
            "--css=styles/epub.css",
            "--toc",
            "--toc-depth=2",
            "--split-level=1",
            "--syntax-highlighting=tango",
            "--resource-path=manuscript:.",
            "--epub-cover-image=assets/cover/cover.png",
        ]
    )
    run([epubcheck, str(output)])
    print(f"Built and validated {output.relative_to(BOOK_ROOT)}")


if __name__ == "__main__":
    main()
