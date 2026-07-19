#!/usr/bin/env python3
"""Regenerate the locked release-target Rust license carrier and manifest."""

from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LICENSES = ROOT / "RUST-DEPENDENCY-LICENSES"
MANIFEST = ROOT / "runtime-license-manifest.json"
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cargo_dependency_graph_sha256(path: Path) -> str:
    """Hash locked third-party packages, excluding release-stamped workspace versions."""

    lock = tomllib.loads(path.read_text())
    packages = [package for package in lock["package"] if "source" in package]
    packages.sort(
        key=lambda package: (package["name"], package["version"], package["source"])
    )
    payload = json.dumps(packages, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def target_license_expression(target: str) -> str:
    """Derive the selected SPDX licenses from cargo-about's target graph."""

    with tempfile.TemporaryDirectory(prefix="basilisk-cargo-about-") as temp:
        output = Path(temp) / "licenses.json"
        subprocess.run(
            [
                "cargo",
                "about",
                "generate",
                "--locked",
                "--fail",
                "--config",
                "about.toml",
                "--manifest-path",
                "crates/basilisk-cli/Cargo.toml",
                "--target",
                target,
                "--format",
                "json",
                "--output-file",
                str(output),
            ],
            cwd=ROOT,
            check=True,
        )
        selected = {entry["id"] for entry in json.loads(output.read_text())["overview"]}
    # Basilisk's MIT code and the Typeshed Apache-2.0/MIT snapshot are also in
    # every wheel even though they are not third-party Cargo packages.
    selected.update(("Apache-2.0", "MIT"))
    return " AND ".join(sorted(selected))


def main() -> None:
    subprocess.run(
        [
            "cargo",
            "about",
            "generate",
            "scripts/runtime-licenses.hbs",
            "--locked",
            "--fail",
            "--manifest-path",
            "crates/basilisk-cli/Cargo.toml",
            "--output-file",
            str(LICENSES),
        ],
        cwd=ROOT,
        check=True,
    )
    manifest = {
        "cargo_dependency_graph_sha256": cargo_dependency_graph_sha256(
            ROOT / "Cargo.lock"
        ),
        "licenses_sha256": sha256(LICENSES),
        "targets": list(TARGETS),
        "wheel_license_expressions": {
            target: target_license_expression(target) for target in TARGETS
        },
    }
    MANIFEST.write_text(f"{json.dumps(manifest, indent=2, sort_keys=True)}\n")


if __name__ == "__main__":
    main()
