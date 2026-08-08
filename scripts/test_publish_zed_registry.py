"""Tests for `scripts/publish_zed_registry.py` — the Zed registry listing edit.

Covers [ZED-MIRROR] (docs/specs/ZED-SPEC.md#ZED-MIRROR). The listing edit runs
against a ~1400-entry third-party file that Basilisk does not own, so the two
properties worth proving are that the Basilisk entry lands correctly *and* that
nothing else in the file moves — a reformatting diff across someone else's
registry is a rejected PR. Network-free by construction: only the pure
text-editing functions are exercised.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from publish_zed_registry import (  # noqa: E402
    EXTENSION_ID,
    SUBMODULE_PATH,
    blocks,
    sort_key,
    splice,
    write_gitmodules,
    write_registry,
)

REGISTRY = """\
[aardvark]
submodule = "extensions/aardvark"
version = "1.0.0"

[basher]
submodule = "extensions/basher"
version = "0.2.1"

[batman]
submodule = "extensions/batman"
version = "3.0.0"

[zig]
submodule = "extensions/zig"
version = "0.9.9"
"""


def entry(version: str) -> list[str]:
    return [
        f"[{EXTENSION_ID}]\n",
        f'submodule = "{SUBMODULE_PATH}"\n',
        f'version = "{version}"\n',
        "\n",
    ]


def keys(text: str) -> list[str]:
    return [h.strip("[]") for h, _ in blocks(text, "[") if h.startswith("[")]


def test_inserts_in_alphabetical_position() -> None:
    out = splice(REGISTRY, "[", f"[{EXTENSION_ID}]", entry("0.41.0"))
    assert keys(out) == ["aardvark", "basher", "basilisk", "batman", "zig"]


def test_leaves_every_other_entry_byte_identical() -> None:
    out = splice(REGISTRY, "[", f"[{EXTENSION_ID}]", entry("0.41.0"))
    before = tomllib.loads(REGISTRY)
    after = {k: v for k, v in tomllib.loads(out).items() if k != EXTENSION_ID}
    assert after == before
    for name in before:
        assert f'[{name}]\nsubmodule = "extensions/{name}"\n' in out


def test_bump_replaces_rather_than_duplicates() -> None:
    first = splice(REGISTRY, "[", f"[{EXTENSION_ID}]", entry("0.41.0"))
    second = splice(first, "[", f"[{EXTENSION_ID}]", entry("0.42.0"))
    assert keys(second).count(EXTENSION_ID) == 1
    assert tomllib.loads(second)[EXTENSION_ID]["version"] == "0.42.0"
    assert keys(second) == keys(first)


def test_splice_is_idempotent() -> None:
    once = splice(REGISTRY, "[", f"[{EXTENSION_ID}]", entry("0.41.0"))
    assert splice(once, "[", f"[{EXTENSION_ID}]", entry("0.41.0")) == once


def test_sort_key_is_case_insensitive_and_unquoted() -> None:
    assert sort_key("[Basilisk]") == "basilisk"
    # `.gitmodules` headers are keyed on the path, after the caller strips the
    # `[submodule ` prefix — the quotes must not survive into the sort order.
    header = '[submodule "extensions/Zig"]'
    assert sort_key(header.removeprefix("[submodule ")) == "extensions/zig"


def test_write_registry_pins_the_version(tmp_path: Path) -> None:
    (tmp_path / "extensions.toml").write_text(REGISTRY, encoding="utf-8")
    write_registry(tmp_path, "0.41.0")
    listed = tomllib.loads((tmp_path / "extensions.toml").read_text(encoding="utf-8"))
    assert listed[EXTENSION_ID] == {"submodule": SUBMODULE_PATH, "version": "0.41.0"}


def test_write_registry_rejects_a_corrupt_result(tmp_path: Path) -> None:
    (tmp_path / "extensions.toml").write_text("[aardvark\nbroken", encoding="utf-8")
    with pytest.raises((SystemExit, tomllib.TOMLDecodeError)):
        write_registry(tmp_path, "0.41.0")


def test_write_gitmodules_sorts_appended_submodules(tmp_path: Path) -> None:
    unsorted = (
        '[submodule "extensions/aardvark"]\n'
        "\tpath = extensions/aardvark\n"
        '[submodule "extensions/zig"]\n'
        "\tpath = extensions/zig\n"
        '[submodule "extensions/basilisk"]\n'
        "\tpath = extensions/basilisk\n"
    )
    (tmp_path / ".gitmodules").write_text(unsorted, encoding="utf-8")
    write_gitmodules(tmp_path)
    ordered = (tmp_path / ".gitmodules").read_text(encoding="utf-8")
    assert ordered.index("aardvark") < ordered.index("basilisk") < ordered.index("zig")
    assert ordered.count("[submodule ") == 3


def test_write_gitmodules_is_idempotent(tmp_path: Path) -> None:
    path = tmp_path / ".gitmodules"
    path.write_text('[submodule "extensions/a"]\n\tpath = extensions/a\n', "utf-8")
    write_gitmodules(tmp_path)
    once = path.read_text(encoding="utf-8")
    write_gitmodules(tmp_path)
    assert path.read_text(encoding="utf-8") == once
