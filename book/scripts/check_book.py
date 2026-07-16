#!/usr/bin/env python3
"""Validate the book manifest, chapter skeleton, sources, and figure ledger."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urlsplit


BOOK_ROOT = Path(__file__).resolve().parents[1]


def load_json(name: str) -> dict[str, Any]:
    """Load a required JSON document from the book root."""
    path = BOOK_ROOT / name
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"Cannot load {name}: {error}") from error
    if not isinstance(value, dict):
        raise SystemExit(f"{name} must contain a JSON object")
    return value


def pandoc_ast(path: Path) -> dict[str, Any]:
    """Parse Markdown through Pandoc rather than approximating Markdown."""
    pandoc = shutil.which("pandoc")
    if pandoc is None:
        raise SystemExit("Pandoc is required for book validation")
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


def inline_text(value: Any) -> str:
    """Convert Pandoc inline nodes into readable text."""
    if isinstance(value, list):
        return "".join(inline_text(item) for item in value)
    if not isinstance(value, dict):
        return ""

    node_type = value.get("t")
    content = value.get("c")
    if node_type == "Str":
        return str(content)
    if node_type in {"Space", "SoftBreak", "LineBreak"}:
        return " "
    if node_type in {"Code", "Math"} and isinstance(content, list):
        return str(content[-1])
    if node_type in {"Link", "Image"} and isinstance(content, list):
        return inline_text(content[1])
    if node_type == "Span" and isinstance(content, list):
        return inline_text(content[1])
    if node_type == "Quoted" and isinstance(content, list):
        return inline_text(content[1])
    return inline_text(content)


def document_text(ast: dict[str, Any]) -> str:
    """Extract visible text from a Pandoc document."""
    return inline_text(ast.get("blocks", []))


def first_heading(ast: dict[str, Any]) -> str | None:
    """Return the first heading's visible text."""
    for block in ast.get("blocks", []):
        if isinstance(block, dict) and block.get("t") == "Header":
            content = block.get("c")
            if isinstance(content, list) and len(content) == 3:
                return inline_text(content[2]).strip()
    return None


def walk_nodes(value: Any) -> list[dict[str, Any]]:
    """Return every Pandoc node in document order."""
    nodes: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if "t" in value:
            nodes.append(value)
        for child in value.values():
            nodes.extend(walk_nodes(child))
    elif isinstance(value, list):
        for child in value:
            nodes.extend(walk_nodes(child))
    return nodes


def image_targets(ast: dict[str, Any]) -> list[tuple[str, str]]:
    """Return image targets and alt text from a Pandoc AST."""
    images: list[tuple[str, str]] = []
    for node in walk_nodes(ast):
        if node.get("t") != "Image":
            continue
        content = node.get("c")
        if not isinstance(content, list) or len(content) != 3:
            continue
        target = content[2]
        if not isinstance(target, list) or not target:
            continue
        images.append((str(target[0]), inline_text(content[1]).strip()))
    return images


def expected_heading(section: dict[str, Any]) -> str:
    """Build the canonical H1 for a manifest section."""
    if section["kind"] == "chapter":
        return f"Chapter {section['number']} — {section['title']}"
    return str(section["title"])


def normalized_local_target(source: Path, target: str) -> Path | None:
    """Resolve a local image target against its Markdown source."""
    split = urlsplit(target)
    if split.scheme or target.startswith("//"):
        return None
    target_path = unquote(split.path)
    if not target_path:
        return source
    return (source.parent / target_path).resolve()


def validate(release: bool) -> list[str]:
    """Return all validation errors without stopping at the first one."""
    errors: list[str] = []
    book = load_json("book.json")
    evidence = load_json("evidence.json")
    figures = load_json("figures.json")
    sources = load_json("sources.json")

    sections = book.get("sections")
    figure_entries = figures.get("figures")
    source_entries = sources.get("sources")
    evidence_entries = evidence.get("chapters")
    if not isinstance(sections, list):
        return ["book.json sections must be an array"]
    if not isinstance(figure_entries, list):
        return ["figures.json figures must be an array"]
    if not isinstance(source_entries, list):
        return ["sources.json sources must be an array"]
    if not isinstance(evidence_entries, list):
        return ["evidence.json chapters must be an array"]

    files = [str(section.get("file", "")) for section in sections]
    if len(files) != len(set(files)):
        errors.append("book.json contains duplicate manuscript files")

    chapter_numbers = [
        section.get("number")
        for section in sections
        if section.get("kind") == "chapter"
    ]
    if chapter_numbers != list(range(1, len(chapter_numbers) + 1)):
        errors.append("Chapter numbers must be contiguous and in reading order")

    expected_evidence_sections = {f"{int(number):02d}" for number in chapter_numbers}
    actual_evidence_sections = {
        str(entry.get("section", "")) for entry in evidence_entries
    }
    if actual_evidence_sections != expected_evidence_sections:
        errors.append("evidence.json must contain exactly one entry for every chapter")
    if len(actual_evidence_sections) != len(evidence_entries):
        errors.append("evidence.json contains duplicate chapter entries")

    targets = book.get("targets", {})
    for field, section_field in (
        ("words", "targetWords"),
        ("printEquivalentPages", "targetPages"),
        ("figures", "targetFigures"),
    ):
        declared = targets.get(field) if isinstance(targets, dict) else None
        measured = sum(int(section.get(section_field, 0)) for section in sections)
        if declared != measured:
            errors.append(
                f"Target {field} is {declared}, but sections total {measured}"
            )

    figure_ids: set[str] = set()
    figure_paths: set[Path] = set()
    figures_by_section: dict[str, int] = {}
    for figure in figure_entries:
        figure_id = str(figure.get("id", ""))
        if not figure_id or figure_id in figure_ids:
            errors.append(f"Missing or duplicate figure id: {figure_id!r}")
        figure_ids.add(figure_id)
        section_key = str(figure.get("section", ""))
        figures_by_section[section_key] = figures_by_section.get(section_key, 0) + 1
        path = BOOK_ROOT / str(figure.get("path", ""))
        figure_paths.add(path.resolve())
        if figure.get("status") == "ready":
            if not path.is_file():
                errors.append(f"Ready figure is missing: {path.relative_to(BOOK_ROOT)}")
            master_value = figure.get("master")
            if not master_value or not (BOOK_ROOT / str(master_value)).is_file():
                errors.append(f"Ready figure has no source master: {figure_id}")
        if len(str(figure.get("alt", "")).split()) < 8:
            errors.append(f"Figure alt text is too weak: {figure_id}")

    if len(figure_entries) != int(targets.get("figures", -1)):
        errors.append("Figure ledger count does not match the book target")

    for section in sections:
        section_key = (
            f"{int(section['number']):02d}"
            if section.get("kind") == "chapter"
            else str(section.get("kind"))
        )
        actual_figures = figures_by_section.get(section_key, 0)
        expected_figures = int(section.get("targetFigures", 0))
        if actual_figures != expected_figures:
            errors.append(
                f"{section['title']} targets {expected_figures} figures but ledger has {actual_figures}"
            )

        path = BOOK_ROOT / str(section["file"])
        if not path.is_file():
            errors.append(f"Missing manuscript file: {section['file']}")
            continue
        ast = pandoc_ast(path)
        heading = first_heading(ast)
        expected = expected_heading(section)
        if heading != expected:
            errors.append(
                f"{section['file']} starts with {heading!r}; expected {expected!r}"
            )

        words = len(document_text(ast).split())
        target_words = int(section.get("targetWords", 0))
        if release and not (target_words * 0.8 <= words <= target_words * 1.15):
            errors.append(
                f"{section['file']} has {words} words; target is {target_words} (80–115% allowed)"
            )

        for target, alt in image_targets(ast):
            resolved = normalized_local_target(path, target)
            if resolved is None:
                errors.append(
                    f"Remote image is forbidden in {section['file']}: {target}"
                )
                continue
            if not resolved.is_file():
                errors.append(
                    f"Missing manuscript image in {section['file']}: {target}"
                )
            if resolved not in figure_paths:
                errors.append(f"Image is absent from figures.json: {target}")
            if len(alt.split()) < 8:
                errors.append(
                    f"Image alt text is too weak in {section['file']}: {target}"
                )

        if release and section.get("status") != "complete":
            errors.append(f"Release section is not complete: {section['file']}")

    keys = [str(source.get("key", "")) for source in source_entries]
    urls = [str(source.get("url", "")) for source in source_entries]
    if len(keys) != len(set(keys)) or "" in keys:
        errors.append("sources.json contains missing or duplicate source keys")
    if len(urls) != len(set(urls)) or "" in urls:
        errors.append("sources.json contains missing or duplicate URLs")
    for source in source_entries:
        split = urlsplit(str(source.get("url", "")))
        if split.scheme != "https" or not split.netloc:
            errors.append(f"Source must use an absolute HTTPS URL: {source.get('key')}")

    cover = figures.get("cover")
    if not isinstance(cover, dict):
        errors.append("figures.json has no cover object")
    else:
        for field in ("master", "path"):
            target = BOOK_ROOT / str(cover.get(field, ""))
            if not target.is_file():
                errors.append(
                    f"Cover {field} is missing: {target.relative_to(BOOK_ROOT)}"
                )

    if release:
        if not book.get("basiliskRelease"):
            errors.append("Release build requires book.json basiliskRelease")
        if any(figure.get("status") != "ready" for figure in figure_entries):
            errors.append("Release build requires every planned figure to be ready")
        for entry in evidence_entries:
            section_key = str(entry.get("section", ""))
            if entry.get("decision") != "publish":
                errors.append(
                    f"Chapter {section_key} has not passed the agreement gate"
                )
            for field in (
                "governingSpecs",
                "releaseImplementation",
                "executableEvidence",
            ):
                value = entry.get(field)
                if not isinstance(value, list) or not value:
                    errors.append(f"Chapter {section_key} has no {field} evidence")

    return errors


def main() -> None:
    """Run validation and present a compact result."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--release", action="store_true", help="enforce publication gates"
    )
    args = parser.parse_args()
    errors = validate(args.release)
    if errors:
        print("Book validation failed:")
        for error in errors:
            print(f"- {error}")
        raise SystemExit(1)
    mode = "release" if args.release else "structural"
    print(f"Book {mode} validation passed.")


if __name__ == "__main__":
    main()
