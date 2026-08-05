#!/usr/bin/env python3
"""Capture Chapter 10 in a real terminal using the pinned Basilisk release."""

from __future__ import annotations

import datetime as dt
import json
import os
import platform
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

import capture_editor_screenshots as release_capture


BOOK_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = BOOK_ROOT / "assets" / "screenshots"
MASTER_DIR = OUTPUT_DIR / "masters"
FIXTURE = BOOK_ROOT / "examples" / "ch10-adoption"
TEST_DRIVER = Path(__file__).with_name("capture_ch10_terminal.test.ts")
BOOK_MANIFEST = BOOK_ROOT / "book.json"
FIGURE_LEDGER = BOOK_ROOT / "figures.json"
CAPTURE_TEST = "Chapter 10 book capture"
PUBLICATION_TRANSFORM = "full 2880x1800 frame uniformly resized to 1600x1000"


def load_json(path: Path) -> dict[str, Any]:
    """Load one required JSON object."""
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise SystemExit(f"Expected a JSON object in {path}")
    return value


def stage_fixture(destination: Path) -> Path:
    """Copy the reviewed fixture, then place it at the pre-fix checkpoint."""
    workspace = destination / "project"
    shutil.copytree(FIXTURE, workspace)
    stages = workspace / "stages"
    decoder = workspace / "src" / "signal_box" / "legacy" / "decoder.py"
    shutil.copy2(decoder, stages / "decoder.reviewed")
    shutil.copy2(stages / "decoder.before", decoder)
    shutil.copy2(stages / "pyproject.before", workspace / "pyproject.toml")
    return workspace


def install_driver(extension: Path) -> None:
    """Add only the capture automation to the pinned release test harness."""
    destination = extension / "src" / "test" / "suite" / TEST_DRIVER.name
    shutil.copy2(TEST_DRIVER, destination)


def capture(
    extension: Path,
    workspace: Path,
    binary_directory: Path,
    capture_dir: Path,
    node: str,
    npx: str,
    editor: str,
) -> None:
    """Run real 0.39.0 commands in a headed VS Code integrated terminal."""
    env = os.environ.copy()
    env.pop("ELECTRON_RUN_AS_NODE", None)
    env.update(
        {
            "BASILISK_SCREENSHOTS": "1",
            "BASILISK_BOOK_CH10_SCREENSHOTS": "1",
            "BASILISK_SCREENSHOT_CDP_PORT": str(release_capture.unused_local_port()),
            "BASILISK_SCREENSHOT_OUTPUT_DIR": str(capture_dir),
            "BASILISK_SCREENSHOT_WORKSPACE": str(workspace),
            "BASILISK_CH10_WORKSPACE": str(workspace),
            "BASILISK_CH10_BINARY_DIR": str(binary_directory),
        }
    )
    watcher = subprocess.Popen(
        [node, "scripts/screenshot-watcher.mjs"], cwd=extension, env=env
    )
    try:
        release_capture.run(
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


def sha256(path: Path) -> str:
    """Return the SHA-256 digest of one file."""
    return release_capture.sha256(path)


def publish(capture_dir: Path, magick: str) -> dict[str, str]:
    """Preserve raw captures and create publication crops without repainting."""
    captured = {
        "fix": capture_dir / "10-cli-fix-full.png",
        "adopt": capture_dir / "10-adopt-status-full.png",
    }
    missing = [path.name for path in captured.values() if not path.is_file()]
    if missing:
        raise SystemExit(f"Screenshot capture did not produce: {', '.join(missing)}")
    MASTER_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    digests: dict[str, str] = {}
    for name, source in captured.items():
        master = MASTER_DIR / source.name
        target_name = "10-cli-fix.png" if name == "fix" else "10-adopt-status.png"
        target = OUTPUT_DIR / target_name
        shutil.copy2(source, master)
        release_capture.run(
            [magick, str(master), "-resize", "1600x1000", "-strip", str(target)],
            BOOK_ROOT,
        )
        digests[name] = sha256(master)
    return digests


def update_capture_hashes(
    digests: dict[str, str], artifact: str, artifact_digest: str, editor: str
) -> None:
    """Mark the two captures ready and record reproducible release provenance."""
    ledger = load_json(FIGURE_LEDGER)
    figures = ledger.get("figures")
    if not isinstance(figures, list):
        raise SystemExit("figures.json has no figures list")
    expected = {
        "shot-10-cli-fix": ("fix", "10-cli-fix-full.png"),
        "shot-10-adopt-status": ("adopt", "10-adopt-status-full.png"),
    }
    updated: set[str] = set()
    for figure in figures:
        if not isinstance(figure, dict) or figure.get("id") not in expected:
            continue
        key, master_name = expected[str(figure["id"])]
        figure["status"] = "ready"
        figure["master"] = f"assets/screenshots/masters/{master_name}"
        figure["capture"] = {
            "authenticity": "direct-release-capture",
            "basiliskVersion": "0.39.0",
            "releaseTag": "v0.39.0",
            "releaseCommit": "b8ae454cfabc54d26d7e4efc029f2f01bd083bc8",
            "releaseArtifact": artifact,
            "releaseArtifactSha256": artifact_digest,
            "rawMaster": f"assets/screenshots/masters/{master_name}",
            "masterSha256": digests[key],
            "fixture": "examples/ch10-adoption",
            "fixtureStaging": "copied into an isolated temporary workspace before capture",
            "editor": f"Visual Studio Code {editor} Extension Development Host",
            "os": f"{platform.system()} {platform.mac_ver()[0]}",
            "architecture": platform.machine(),
            "theme": "Dark Modern",
            "viewport": "1440x900 CSS pixels at 2x device scale",
            "method": (
                "Actual commands executed in a VS Code integrated terminal; "
                "headed workbench captured with CDP Page.captureScreenshot by "
                "scripts/capture_adoption_screenshots.py"
            ),
            "capturedAt": dt.date.today().isoformat(),
            "crop": PUBLICATION_TRANSFORM,
        }
        updated.add(str(figure["id"]))
    if updated != set(expected):
        raise SystemExit("figures.json is missing a Chapter 10 screenshot entry")
    FIGURE_LEDGER.write_text(
        json.dumps(ledger, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def main() -> None:
    """Download, verify, execute, capture, crop, and record Chapter 10."""
    npm = release_capture.require_tool("npm")
    node = release_capture.require_tool("node")
    npx = release_capture.require_tool("npx")
    magick = release_capture.require_tool("magick")
    if not TEST_DRIVER.is_file() or not (FIXTURE / "pyproject.toml").is_file():
        raise SystemExit("Chapter 10 capture driver or fixture is incomplete")

    book = load_json(BOOK_MANIFEST)
    version = str(book.get("basiliskRelease", ""))
    tag = str(book.get("basiliskReleaseTag", ""))
    commit = str(book.get("basiliskReleaseCommit", ""))
    editor = str(book.get("screenshotCapture", {}).get("editorVersion", ""))
    platform_key = release_capture.platform_key()
    artifact, expected_digest = release_capture.checked_artifact(book, platform_key)
    if version != "0.39.0" or not tag or len(commit) != 40 or not editor:
        raise SystemExit("book.json release or screenshot editor pin is incomplete")

    with (
        tempfile.TemporaryDirectory(prefix=f"basilisk-book-ch10-{version}-") as temporary,
        tempfile.TemporaryDirectory(prefix="bsk-ch10-", dir="/tmp") as fixture_temporary,
    ):
        work = Path(temporary)
        checksums = work / "checksums-sha256.txt"
        vsix = work / artifact
        source_archive = work / "source.tar.gz"
        release_base = f"https://github.com/Nimblesite/Basilisk/releases/download/{tag}"
        release_capture.download(f"{release_base}/checksums-sha256.txt", checksums)
        if release_capture.published_checksum(checksums, artifact) != expected_digest:
            raise SystemExit("book.json VSIX checksum does not match the published ledger")
        release_capture.download(f"{release_base}/{artifact}", vsix)
        if sha256(vsix) != expected_digest:
            raise SystemExit("Downloaded VSIX failed its published SHA-256")
        release_capture.download(
            f"https://github.com/Nimblesite/Basilisk/archive/{commit}.tar.gz",
            source_archive,
        )

        source_extension = release_capture.extract_source(source_archive, work / "source")
        release_extension = release_capture.extract_vsix(vsix, work / "vsix")
        release_capture.verify_release_extension(release_extension, version, platform_key)
        install_driver(source_extension)
        release_capture.run([npm, "ci"], source_extension)
        release_capture.run([npm, "run", "compile"], source_extension)
        release_capture.overlay_release_product(source_extension, release_extension)
        workspace = stage_fixture(Path(fixture_temporary))
        capture_dir = work / "captures"
        capture_dir.mkdir()
        binary_directory = source_extension / "bin" / platform_key
        capture(
            source_extension,
            workspace,
            binary_directory,
            capture_dir,
            node,
            npx,
            editor,
        )
        digests = publish(capture_dir, magick)
        update_capture_hashes(digests, artifact, expected_digest, editor)

    print(f"Captured real Basilisk {version} Chapter 10 terminal screenshots.")
    print(f"Verified release artifact SHA-256: {expected_digest}")


if __name__ == "__main__":
    main()
