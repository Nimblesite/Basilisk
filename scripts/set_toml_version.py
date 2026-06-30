#!/usr/bin/env python3
"""Structurally set a TOML file's package version ([SWR-VERSION-BUILD-STAMPING] §3.3).

Used by render-zed-mirror.sh so the Zed-mirror version stamping goes through a
real TOML parser instead of blind `sed`. `tomllib` validates the file is
well-formed before the edit and confirms the version landed after it; the write
itself replaces the single `version = ...` (or `version.workspace = ...`) line in
the package scope, preserving the file's formatting and comments.

Handles all three rendered carriers uniformly:
  * the extension `Cargo.toml`     — `[package]` → `version = "0.0.0-PLACEHOLDER"`
  * the vendored crate `Cargo.toml`— `[package]` → `version.workspace = true`
  * `extension.toml`               — top-level   → `version = "0.0.0-PLACEHOLDER"`

Usage:
    set_toml_version.py <file> <version>
"""

from __future__ import annotations

import re
import sys
import tomllib

VERSION_LINE = re.compile(r"^version(\.workspace)?\s*=.*$", re.MULTILINE)


def package_version(doc: dict) -> object:
    pkg = doc.get("package")
    if isinstance(pkg, dict) and "version" in pkg:
        return pkg["version"]
    return doc.get("version")


def main(file: str, version: str) -> int:
    with open(file, encoding="utf-8") as handle:
        text = handle.read()

    # Validate the file parses and actually carries a version key to replace.
    if package_version(tomllib.loads(text)) is None:
        print(f"set_toml_version: no version key in {file}", file=sys.stderr)
        return 2

    stamped, count = VERSION_LINE.subn(f'version = "{version}"', text, count=1)
    if count != 1:
        print(f"set_toml_version: no version line matched in {file}", file=sys.stderr)
        return 2

    # Re-parse: the result is still valid TOML and the version is exactly ours.
    if package_version(tomllib.loads(stamped)) != version:
        print(
            f"set_toml_version: post-write verification failed for {file}",
            file=sys.stderr,
        )
        return 2

    with open(file, "w", encoding="utf-8") as handle:
        handle.write(stamped)
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: set_toml_version.py <file> <version>", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
