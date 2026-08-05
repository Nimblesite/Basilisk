#!/usr/bin/env python3
"""Capture Chapter 9 from the pinned, checksum-verified Basilisk release."""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import platform
import shutil
import socket
import stat
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen


BOOK_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = BOOK_ROOT / "assets" / "screenshots"
MASTER_DIR = OUTPUT_DIR / "masters"
WORKSPACE = BOOK_ROOT / "examples" / "signal-box"
BOOK_MANIFEST = BOOK_ROOT / "book.json"
FIGURE_LEDGER = BOOK_ROOT / "figures.json"
CAPTURE_TEST = "configuration editor tag-first rules"
FULL_EDITOR = "09-configuration-editor-full.png"
FULL_PREVIEW = "09-configuration-preview-full.png"
CROP_GEOMETRY = "2100x1312+90+130"


def load_json(path: Path) -> dict[str, Any]:
    """Load a required JSON object."""
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise SystemExit(f"Expected a JSON object in {path}")
    return value


def require_tool(name: str) -> str:
    """Return an executable path or fail with a clear dependency error."""
    executable = shutil.which(name)
    if executable is None:
        raise SystemExit(f"Required screenshot tool is missing: {name}")
    return executable


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    """Run one checked capture prerequisite."""
    subprocess.run(command, cwd=cwd, env=env, check=True)


def download(url: str, target: Path) -> None:
    """Download one official release input over HTTPS."""
    request = Request(url, headers={"User-Agent": "Basilisk-book-capture"})
    with urlopen(request, timeout=120) as response, target.open("wb") as output:
        shutil.copyfileobj(response, output)


def sha256(path: Path) -> str:
    """Return the SHA-256 digest of one file."""
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def platform_key() -> str:
    """Return the release artifact platform key for this host."""
    system = platform.system()
    machine = platform.machine().lower()
    aliases = {"aarch64": "arm64", "amd64": "x64", "x86_64": "x64"}
    architecture = aliases.get(machine, machine)
    systems = {"Darwin": "darwin", "Linux": "linux", "Windows": "win32"}
    if system not in systems:
        raise SystemExit(f"Unsupported screenshot host: {system} {machine}")
    return f"{systems[system]}-{architecture}"


def checked_artifact(book: dict[str, Any], key: str) -> tuple[str, str]:
    """Return the pinned VSIX name and checksum for this platform."""
    capture = book.get("screenshotCapture")
    if not isinstance(capture, dict):
        raise SystemExit("book.json has no screenshotCapture record")
    artifacts = capture.get("releaseArtifacts")
    if not isinstance(artifacts, dict) or not isinstance(artifacts.get(key), dict):
        raise SystemExit(f"book.json has no verified screenshot artifact for {key}")
    record = artifacts[key]
    name = str(record.get("name", ""))
    digest = str(record.get("sha256", ""))
    if not name or len(digest) != 64:
        raise SystemExit(f"Invalid screenshot artifact record for {key}")
    return name, digest


def published_checksum(checksums: Path, artifact: str) -> str:
    """Read one artifact digest from the published checksum ledger."""
    for line in checksums.read_text(encoding="utf-8").splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1].removeprefix("./") == artifact:
            return fields[0]
    raise SystemExit(f"Published checksums do not contain {artifact}")


def extract_source(archive_path: Path, destination: Path) -> Path:
    """Extract the pinned commit archive and return its extension directory."""
    with tarfile.open(archive_path, "r:gz") as archive:
        archive.extractall(destination, filter="data")
    candidates = [
        child / "vscode-extension"
        for child in destination.iterdir()
        if child.is_dir() and (child / "vscode-extension").is_dir()
    ]
    if len(candidates) != 1:
        raise SystemExit("Pinned source archive did not contain one extension tree")
    return candidates[0]


def extract_vsix(vsix_path: Path, destination: Path) -> Path:
    """Extract a checksum-verified VSIX and return its extension directory."""
    destination_root = destination.resolve()
    with zipfile.ZipFile(vsix_path) as archive:
        for member in archive.infolist():
            target = (destination / member.filename).resolve()
            if target != destination_root and destination_root not in target.parents:
                raise SystemExit(f"Unsafe path in release VSIX: {member.filename}")
        archive.extractall(destination)
    extension = destination / "extension"
    if not extension.is_dir():
        raise SystemExit("Release VSIX has no extension directory")
    return extension


def verify_release_extension(extension: Path, version: str, key: str) -> None:
    """Reject an artifact whose package or bundled binary version is wrong."""
    package = load_json(extension / "package.json")
    if package.get("version") != version:
        raise SystemExit(
            f"VSIX version is {package.get('version')}, expected {version}"
        )
    executable = "basilisk.exe" if key.startswith("win32-") else "basilisk"
    binary = extension / "bin" / key / executable
    if not key.startswith("win32-"):
        for bundled_binary in binary.parent.iterdir():
            if bundled_binary.is_file():
                mode = bundled_binary.stat().st_mode
                bundled_binary.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    result = subprocess.run(
        [str(binary), "--version"], check=True, capture_output=True, text=True
    )
    if f"basilisk {version}" not in result.stdout:
        raise SystemExit(f"Bundled binary does not report basilisk {version}")


def overlay_release_product(source: Path, release: Path) -> None:
    """Keep the tag's test driver while running the shipped product bytes."""
    for directory in ("out", "bin"):
        shutil.copytree(release / directory, source / directory, dirs_exist_ok=True)
    for filename in ("package.json", "shipwright.json"):
        shutil.copy2(release / filename, source / filename)


def unused_local_port() -> int:
    """Reserve and release an ephemeral loopback port for the isolated host."""
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def capture(
    extension: Path, capture_dir: Path, node: str, npx: str, editor: str
) -> None:
    """Drive the real LSP snapshot and preview in an isolated headed VS Code."""
    env = os.environ.copy()
    env.pop("ELECTRON_RUN_AS_NODE", None)
    env.update(
        {
            "BASILISK_SCREENSHOTS": "1",
            "BASILISK_BOOK_SCREENSHOTS": "1",
            "BASILISK_SCREENSHOT_CDP_PORT": str(unused_local_port()),
            "BASILISK_SCREENSHOT_OUTPUT_DIR": str(capture_dir),
            "BASILISK_SCREENSHOT_WORKSPACE": str(WORKSPACE),
        }
    )
    watcher = subprocess.Popen(
        [node, "scripts/screenshot-watcher.mjs"], cwd=extension, env=env
    )
    try:
        run(
            [npx, "vscode-test", "--code-version", editor, "--grep", CAPTURE_TEST],
            extension,
            env,
        )
    finally:
        watcher.terminate()
        try:
            watcher.wait(timeout=10)
        except subprocess.TimeoutExpired:
            watcher.kill()
            watcher.wait()


def publish(capture_dir: Path, magick: str) -> dict[str, str]:
    """Preserve untouched masters and produce deterministic publication crops."""
    captured = {
        "editor": capture_dir / FULL_EDITOR,
        "preview": capture_dir / FULL_PREVIEW,
    }
    missing = [path.name for path in captured.values() if not path.is_file()]
    if missing:
        raise SystemExit(f"Screenshot capture did not produce: {', '.join(missing)}")
    MASTER_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    digests: dict[str, str] = {}
    for name, source in captured.items():
        master = MASTER_DIR / source.name
        target = OUTPUT_DIR / f"09-configuration-{name}.png"
        shutil.copy2(source, master)
        run(
            [
                magick,
                str(master),
                "-crop",
                CROP_GEOMETRY,
                "+repage",
                "-resize",
                "1600x1000",
                "-strip",
                str(target),
            ],
            BOOK_ROOT,
        )
        digests[name] = sha256(master)
    return digests


def update_capture_hashes(digests: dict[str, str]) -> None:
    """Update only the two raw-master hashes after a successful recapture."""
    ledger = load_json(FIGURE_LEDGER)
    figures = ledger.get("figures")
    if not isinstance(figures, list):
        raise SystemExit("figures.json has no figures list")
    expected = {
        "shot-09-config-editor": digests["editor"],
        "shot-09-config-preview": digests["preview"],
    }
    updated: set[str] = set()
    for figure in figures:
        if not isinstance(figure, dict) or figure.get("id") not in expected:
            continue
        capture_record = figure.get("capture")
        if not isinstance(capture_record, dict):
            raise SystemExit(f"{figure.get('id')} has no capture provenance")
        capture_record["masterSha256"] = expected[str(figure["id"])]
        capture_record["capturedAt"] = dt.date.today().isoformat()
        updated.add(str(figure["id"]))
    if updated != set(expected):
        raise SystemExit("figures.json is missing a Chapter 9 screenshot entry")
    FIGURE_LEDGER.write_text(
        json.dumps(ledger, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def main() -> None:
    """Download, verify, drive, capture, crop, and record the pinned release."""
    npm = require_tool("npm")
    node = require_tool("node")
    npx = require_tool("npx")
    magick = require_tool("magick")
    if not (WORKSPACE / "pyproject.toml").is_file():
        raise SystemExit("Signal Box screenshot workspace is incomplete")

    book = load_json(BOOK_MANIFEST)
    version = str(book.get("basiliskRelease", ""))
    tag = str(book.get("basiliskReleaseTag", ""))
    commit = str(book.get("basiliskReleaseCommit", ""))
    editor = str(book.get("screenshotCapture", {}).get("editorVersion", ""))
    key = platform_key()
    artifact, expected_digest = checked_artifact(book, key)
    if not version or not tag or len(commit) != 40 or not editor:
        raise SystemExit("book.json release or screenshot editor pin is incomplete")

    with tempfile.TemporaryDirectory(prefix=f"basilisk-book-{version}-") as temporary:
        work = Path(temporary)
        checksums = work / "checksums-sha256.txt"
        vsix = work / artifact
        source_archive = work / "source.tar.gz"
        release_base = f"https://github.com/Nimblesite/Basilisk/releases/download/{tag}"
        download(f"{release_base}/checksums-sha256.txt", checksums)
        if published_checksum(checksums, artifact) != expected_digest:
            raise SystemExit(
                "book.json VSIX checksum does not match the published ledger"
            )
        download(f"{release_base}/{artifact}", vsix)
        if sha256(vsix) != expected_digest:
            raise SystemExit("Downloaded VSIX failed its published SHA-256")
        download(
            f"https://github.com/Nimblesite/Basilisk/archive/{commit}.tar.gz",
            source_archive,
        )

        source_extension = extract_source(source_archive, work / "source")
        release_extension = extract_vsix(vsix, work / "vsix")
        verify_release_extension(release_extension, version, key)
        run([npm, "ci"], source_extension)
        run([npm, "run", "compile"], source_extension)
        overlay_release_product(source_extension, release_extension)
        capture_dir = work / "captures"
        capture_dir.mkdir()
        capture(source_extension, capture_dir, node, npx, editor)
        digests = publish(capture_dir, magick)
        update_capture_hashes(digests)

    print(f"Captured real Basilisk {version} Chapter 9 screenshots from {artifact}.")
    print(f"Verified release artifact SHA-256: {expected_digest}")


if __name__ == "__main__":
    main()
