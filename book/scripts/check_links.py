#!/usr/bin/env python3
"""Check Markdown targets, local fragments, source authority, and external URLs."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import unquote, urlsplit
from urllib.request import Request, urlopen


BOOK_ROOT = Path(__file__).resolve().parents[1]
USER_AGENT = "Mozilla/5.0 (compatible; The-Basilisk-Book-Link-Audit/1.0)"


@dataclass(frozen=True)
class Reference:
    """One link or image found in a Markdown source."""

    source: Path
    target: str
    kind: str


class AnchorParser(HTMLParser):
    """Collect HTML id and legacy name anchors."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.anchors: set[str] = set()

    def handle_starttag(self, _tag: str, attrs: list[tuple[str, str | None]]) -> None:
        for name, value in attrs:
            if name in {"id", "name"} and value:
                self.anchors.add(value)


def pandoc_ast(path: Path) -> dict[str, Any]:
    """Parse one Markdown file through Pandoc."""
    pandoc = shutil.which("pandoc")
    if pandoc is None:
        raise SystemExit("Pandoc is required for Markdown link validation")
    result = subprocess.run(
        [pandoc, "--from=gfm", "--to=json", str(path)],
        cwd=BOOK_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    value = json.loads(result.stdout)
    if not isinstance(value, dict):
        raise SystemExit(f"Pandoc returned an invalid AST for {path}")
    return value


def walk(value: Any) -> list[dict[str, Any]]:
    """Return every Pandoc node below a value."""
    nodes: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if "t" in value:
            nodes.append(value)
        for child in value.values():
            nodes.extend(walk(child))
    elif isinstance(value, list):
        for child in value:
            nodes.extend(walk(child))
    return nodes


def references(path: Path, ast: dict[str, Any]) -> list[Reference]:
    """Extract Markdown links and images from an AST."""
    found: list[Reference] = []
    for node in walk(ast):
        node_type = node.get("t")
        if node_type not in {"Link", "Image"}:
            continue
        content = node.get("c")
        if not isinstance(content, list) or len(content) != 3:
            continue
        target = content[2]
        if not isinstance(target, list) or not target:
            continue
        found.append(
            Reference(
                source=path,
                target=str(target[0]),
                kind="image" if node_type == "Image" else "link",
            )
        )
    return found


def header_ids(path: Path, cache: dict[Path, set[str]]) -> set[str]:
    """Return Pandoc-generated header identifiers for a Markdown file."""
    resolved = path.resolve()
    if resolved in cache:
        return cache[resolved]
    identifiers: set[str] = set()
    for node in walk(pandoc_ast(resolved)):
        if node.get("t") != "Header":
            continue
        content = node.get("c")
        if not isinstance(content, list) or len(content) != 3:
            continue
        attributes = content[1]
        if isinstance(attributes, list) and attributes:
            identifier = str(attributes[0])
            if identifier:
                identifiers.add(identifier)
    cache[resolved] = identifiers
    return identifiers


def validate_local(
    reference: Reference, anchor_cache: dict[Path, set[str]]
) -> str | None:
    """Return an error for a broken local target or fragment."""
    split = urlsplit(reference.target)
    target_path = unquote(split.path)
    path = (
        reference.source if not target_path else reference.source.parent / target_path
    )
    path = path.resolve()
    if not path.exists():
        return (
            f"{reference.source.relative_to(BOOK_ROOT)} -> {reference.target} (missing)"
        )
    if reference.kind == "image" and not path.is_file():
        return f"{reference.source.relative_to(BOOK_ROOT)} -> {reference.target} (not a file)"
    if split.fragment:
        if path.suffix.lower() not in {".md", ".markdown"}:
            return (
                f"{reference.source.relative_to(BOOK_ROOT)} -> {reference.target} "
                "(cannot validate fragment on a non-Markdown local target)"
            )
        if unquote(split.fragment) not in header_ids(path, anchor_cache):
            return f"{reference.source.relative_to(BOOK_ROOT)} -> {reference.target} (missing fragment)"
    return None


def fetch_external(url: str) -> tuple[str, str | None]:
    """Fetch one URL and validate its fragment when HTML supplies anchors."""
    last_error: str | None = None
    for attempt in range(3):
        request = Request(
            url, headers={"User-Agent": USER_AGENT, "Accept": "text/html,*/*"}
        )
        try:
            with urlopen(request, timeout=20) as response:
                status = getattr(response, "status", 200)
                if status < 200 or status >= 400:
                    return url, f"HTTP {status}"
                body = response.read(5_000_000)
                final_url = response.geturl()
                content_type = response.headers.get_content_type()
                fragment = urlsplit(url).fragment
                if fragment and content_type in {"text/html", "application/xhtml+xml"}:
                    parser = AnchorParser()
                    parser.feed(
                        body.decode(
                            response.headers.get_content_charset() or "utf-8", "replace"
                        )
                    )
                    if unquote(fragment) not in parser.anchors:
                        return url, f"fragment #{fragment} not found"
                if (
                    final_url != url
                    and urlsplit(final_url)._replace(fragment="").geturl()
                    != urlsplit(url)._replace(fragment="").geturl()
                ):
                    return url, f"redirected to {final_url}"
                return url, None
        except HTTPError as error:
            last_error = f"HTTP {error.code}"
        except (URLError, TimeoutError, OSError) as error:
            last_error = str(error)
        if attempt < 2:
            time.sleep(0.5 * (attempt + 1))
    return url, last_error or "unknown network failure"


def main() -> None:
    """Run the local, authority, and optional external link audits."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--external", action="store_true", help="fetch every external URL"
    )
    args = parser.parse_args()

    markdown_files = sorted(
        path
        for path in BOOK_ROOT.rglob("*.md")
        if "dist" not in path.relative_to(BOOK_ROOT).parts
    )
    all_references: list[Reference] = []
    for path in markdown_files:
        all_references.extend(references(path, pandoc_ast(path)))

    source_data = json.loads((BOOK_ROOT / "sources.json").read_text(encoding="utf-8"))
    approved_urls = {str(source["url"]) for source in source_data["sources"]}
    errors: list[str] = []
    anchor_cache: dict[Path, set[str]] = {}
    external_urls: set[str] = set(approved_urls)

    for reference in all_references:
        split = urlsplit(reference.target)
        if split.scheme in {"http", "https"}:
            external_urls.add(reference.target)
            relative = reference.source.relative_to(BOOK_ROOT)
            if relative.parts and relative.parts[0] == "manuscript":
                if reference.target not in approved_urls:
                    errors.append(
                        f"{relative} cites an external URL absent from sources.json: {reference.target}"
                    )
            continue
        if split.scheme in {"mailto", "tel"}:
            continue
        if split.scheme or reference.target.startswith("//"):
            errors.append(
                f"{reference.source.relative_to(BOOK_ROOT)} uses unsupported target: {reference.target}"
            )
            continue
        local_error = validate_local(reference, anchor_cache)
        if local_error:
            errors.append(local_error)

    if args.external:
        with ThreadPoolExecutor(max_workers=10) as executor:
            futures = {
                executor.submit(fetch_external, url): url
                for url in sorted(external_urls)
            }
            for future in as_completed(futures):
                url, error = future.result()
                if error:
                    errors.append(f"{url} ({error})")

    if errors:
        print("Link audit failed:")
        for error in sorted(errors):
            print(f"- {error}")
        raise SystemExit(1)

    external_note = f" and {len(external_urls)} external URLs" if args.external else ""
    print(
        f"Link audit passed: {len(markdown_files)} Markdown files, "
        f"{len(all_references)} references{external_note}."
    )


if __name__ == "__main__":
    main()
