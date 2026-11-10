#!/usr/bin/env python3
"""Real-world smoke test for [STUBRES-CUSTOM-TYPESHED].

Points ``typeshed-path`` at an unmodified, pinned ``micropython-stdlib-stubs``
release — the exact tree the feature's reporter (Jos Verlinde, maintainer of
``micropython-stubs``) ships — and drives the real ``basilisk`` binary over it.

Spec: docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED
(custom typeshed acceptance: "MicroPython
      real-tree smoke test").

It proves the two behaviours the reporter asked for, as an A/B contrast whose
only variable is ``typeshed-path``:

  A. With ``typeshed-path`` = the real MicroPython tree
     * ``from os import ilistdir`` resolves — ``ilistdir`` is a MicroPython-only
       symbol absent from CPython's ``os``, so resolving it proves the custom
       tree is genuinely canonical for stdlib (typing-spec step 3).
     * ``from collections import namedtuple, OrderedDict`` resolves against the
       MicroPython ``collections`` stub.
     * ``import pathlib`` / ``from fractions import Fraction`` — present in
       CPython's stdlib but ABSENT from the partial MicroPython tree — fall
       through to ``imports_unresolved``. This is the load-bearing canonicality
       point: the bundled name-set must NOT rescue them.

  B. Control: the same source with no ``typeshed-path``
     * The bundled CPython name-set recognises ``pathlib`` / ``fractions``, so
       they do NOT report ``imports_unresolved`` — confirming ``typeshed-path``
       is the sole cause of the fall-through in A.

Network: downloads one wheel from PyPI via ``pip``. Not wired into the blocking
CI matrix (PyPI availability must never gate a merge); run it on demand with
``make _smoke_micropython``.

Usage:
  python3 scripts/smoke_micropython_typeshed.py [--bin PATH] [--version VER]
"""

from __future__ import annotations

import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile
import zipfile

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_BIN = REPO_ROOT / "target" / "debug" / "basilisk"
# Pinned so the smoke test is reproducible; bump deliberately, never floating.
DEFAULT_VERSION = "1.28.0.post5"
# Modules present in the real MicroPython tree that must resolve against it.
RESOLVES = ("os", "collections")
# CPython-stdlib modules absent from the partial MicroPython tree.
FALLS_THROUGH = ("pathlib", "fractions")

APP_SOURCE = """\
from os import ilistdir
from collections import namedtuple, OrderedDict
import pathlib
from fractions import Fraction

entries = ilistdir("/")
Point = namedtuple("Point", "x y")
cache: OrderedDict = OrderedDict()
p = pathlib.Path(".")
half = Fraction(1, 2)
"""


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=pathlib.Path, default=DEFAULT_BIN)
    parser.add_argument("--version", default=DEFAULT_VERSION)
    return parser.parse_args(argv)


def locate_binary(explicit: pathlib.Path) -> pathlib.Path:
    """Return the basilisk binary, building the debug binary if it is absent."""
    if explicit.exists():
        return explicit
    if explicit != DEFAULT_BIN:
        raise FileNotFoundError(f"basilisk binary not found: {explicit}")
    subprocess.run(
        ["cargo", "build", "-p", "basilisk-cli", "--bin", "basilisk"],
        cwd=REPO_ROOT,
        check=True,
    )
    return explicit


def download_stub_tree(version: str, dest: pathlib.Path) -> pathlib.Path:
    """Download + extract the pinned micropython-stdlib-stubs wheel; return root."""
    spec = f"micropython-stdlib-stubs=={version}"
    subprocess.run(
        [sys.executable, "-m", "pip", "download", spec, "--no-deps", "-d", str(dest)],
        check=True,
    )
    wheels = sorted(dest.glob("micropython_stdlib_stubs-*.whl"))
    if not wheels:
        raise FileNotFoundError(f"no wheel downloaded for {spec}")
    tree = dest / "tree"
    with zipfile.ZipFile(wheels[0]) as archive:
        archive.extractall(tree)
    stdlib = tree / "stdlib"
    if not stdlib.is_dir():
        raise FileNotFoundError(f"wheel has no stdlib/ directory: {stdlib}")
    return tree


def write_project(root: pathlib.Path, typeshed: pathlib.Path | None) -> None:
    """Lay out a one-file project, optionally with typeshed-path configured."""
    block = f'\n\n[tool.basilisk]\ntypeshed-path = "{typeshed}"\n' if typeshed else "\n"
    (root / "pyproject.toml").write_text(
        f'[project]\nname = "mp-smoke"\nversion = "0.1.0"{block}'
    )
    (root / "app.py").write_text(APP_SOURCE)


def unresolved_modules(stdout: str) -> set[str]:
    """Module names Basilisk reported as ``imports_unresolved`` (no regex)."""
    marker = "Cannot resolve import `"
    names: set[str] = set()
    for line in stdout.splitlines():
        if marker in line:
            names.add(line.split(marker, 1)[1].split("`", 1)[0])
    return names


def run_check(binary: pathlib.Path, project: pathlib.Path) -> str:
    result = subprocess.run(
        [str(binary), "check", "app.py"],
        cwd=project,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout


def check_real_tree(
    binary: pathlib.Path, typeshed: pathlib.Path, work: pathlib.Path
) -> list[str]:
    """Run A: real MicroPython tree — assert resolve + canonical fall-through."""
    project = work / "with_typeshed"
    project.mkdir()
    write_project(project, typeshed)
    unresolved = unresolved_modules(run_check(binary, project))
    failures: list[str] = []
    for module in RESOLVES:
        if module in unresolved:
            failures.append(
                f"[A] `{module}` should resolve against the MicroPython tree"
            )
    for module in FALLS_THROUGH:
        if module not in unresolved:
            failures.append(
                f"[A] `{module}` (absent from MicroPython tree) must fall through"
            )
    return failures


def check_control(binary: pathlib.Path, work: pathlib.Path) -> list[str]:
    """Run B: no typeshed-path — the bundled name-set must rescue the absentees."""
    project = work / "control"
    project.mkdir()
    write_project(project, None)
    unresolved = unresolved_modules(run_check(binary, project))
    return [
        f"[B] `{module}` must NOT be unresolved without typeshed-path (bundled rescue)"
        for module in FALLS_THROUGH
        if module in unresolved
    ]


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    binary = locate_binary(args.bin)
    work = pathlib.Path(tempfile.mkdtemp(prefix="bsk_mp_smoke_"))
    try:
        tree = download_stub_tree(args.version, work / "download")
        print(f"real tree: micropython-stdlib-stubs=={args.version} -> {tree}")
        failures = check_real_tree(binary, tree, work) + check_control(binary, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    if failures:
        print("SMOKE TEST FAILED:")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print(
        "SMOKE TEST PASSED: MicroPython os.ilistdir + collections resolve; "
        "pathlib/fractions fall through per canonicality; bundled control rescues them."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
