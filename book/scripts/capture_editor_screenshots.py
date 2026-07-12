#!/usr/bin/env python3
"""Capture Chapter 9 from the real VS Code extension and Basilisk LSP."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path


BOOK_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = BOOK_ROOT.parent
EXTENSION_ROOT = REPO_ROOT / "vscode-extension"
OUTPUT_DIR = BOOK_ROOT / "assets" / "screenshots"
MASTER_DIR = OUTPUT_DIR / "masters"
WORKSPACE = BOOK_ROOT / "examples" / "signal-box"


def require_tool(name: str) -> str:
    """Return an executable path or fail with a clear dependency error."""
    executable = shutil.which(name)
    if executable is None:
        raise SystemExit(f"Required screenshot tool is missing: {name}")
    return executable


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    """Run one checked capture prerequisite."""
    subprocess.run(command, cwd=cwd, env=env, check=True)


def main() -> None:
    """Build, stage, launch, and capture the two Chapter 9 editor states."""
    cargo = require_tool("cargo")
    node = require_tool("node")
    npm = require_tool("npm")
    npx = require_tool("npx")
    magick = require_tool("magick")
    if not (WORKSPACE / "pyproject.toml").is_file():
        raise SystemExit("Signal Box screenshot workspace is incomplete")

    run(
        [cargo, "build", "-p", "basilisk-cli", "-p", "basilisk-profiler-helper"],
        REPO_ROOT,
    )
    run(
        [node, str(EXTENSION_ROOT / "scripts" / "stage-runtime.mjs"), "target/debug"],
        REPO_ROOT,
    )
    shutil.copy2(REPO_ROOT / "shipwright.json", EXTENSION_ROOT / "shipwright.json")
    run([npm, "run", "compile"], EXTENSION_ROOT)

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    for stale in [*OUTPUT_DIR.glob("*.signal"), *OUTPUT_DIR.glob("*.tmp-*.png")]:
        stale.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(
        {
            "BASILISK_SCREENSHOTS": "1",
            "BASILISK_BOOK_SCREENSHOTS": "1",
            "BASILISK_SCREENSHOT_CDP_PORT": "9239",
            "BASILISK_SCREENSHOT_OUTPUT_DIR": str(OUTPUT_DIR),
            "BASILISK_SCREENSHOT_WORKSPACE": str(WORKSPACE),
        }
    )
    watcher = subprocess.Popen(
        [node, "scripts/screenshot-watcher.mjs"], cwd=EXTENSION_ROOT, env=env
    )
    try:
        run(
            [
                npx,
                "vscode-test",
                "--grep",
                "configuration editor tag-first rules",
            ],
            EXTENSION_ROOT,
            env,
        )
    finally:
        watcher.terminate()
        try:
            watcher.wait(timeout=10)
        except subprocess.TimeoutExpired:
            watcher.kill()
            watcher.wait()

    captured = [
        OUTPUT_DIR / "09-configuration-editor-full.png",
        OUTPUT_DIR / "09-configuration-preview-full.png",
    ]
    missing = [path.name for path in captured if not path.is_file()]
    if missing:
        raise SystemExit(f"Screenshot capture did not produce: {', '.join(missing)}")

    MASTER_DIR.mkdir(parents=True, exist_ok=True)
    editor_master = MASTER_DIR / captured[0].name
    preview_master = MASTER_DIR / captured[1].name
    captured[0].replace(editor_master)
    captured[1].replace(preview_master)
    crops = [
        (editor_master, "2130x1331+90+130", OUTPUT_DIR / "09-configuration-editor.png"),
        (preview_master, "1600x1000+680+370", OUTPUT_DIR / "09-configuration-preview.png"),
    ]
    for master, geometry, target in crops:
        run(
            [
                magick,
                str(master),
                "-crop",
                geometry,
                "+repage",
                "-resize",
                "1600x1000!",
                "-strip",
                str(target),
            ],
            REPO_ROOT,
        )
    print("Captured real Chapter 9 VS Code screenshots.")


if __name__ == "__main__":
    main()
