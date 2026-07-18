#!/usr/bin/env python3
"""Verify exact legal files in shipped archives ([STUBRES-TYPESHED-LICENSE])."""

from __future__ import annotations

import argparse
import hashlib
import json
import tarfile
import tomllib
import zipfile
from pathlib import Path, PurePosixPath


LEGAL_FILES = ("LICENSE", "NOTICES", "THIRD-PARTY-LICENSES")
TYPESHED_RUNTIME_LICENSE_PACKAGES = {
    "ring": "0.17.14",
    "rustls": "0.23.42",
    "rustls-pki-types": "1.15.0",
    "rustls-webpki": "0.103.13",
    "subtle": "2.6.1",
    "untrusted": "0.9.0",
    "ureq": "3.3.0",
    "ureq-proto": "0.6.0",
    "utf8-zero": "0.8.1",
    "webpki-roots": "1.0.9",
    "zip": "5.1.1",
}

TYPESHED_RUNTIME_LICENSE_SECTIONS = {
    "subtle": (
        "subtle — BSD-3-Clause license",
        "zip — MIT license",
        "4ebb6f223513d064ec60c4e60769ce322df664f6f9712622e08a358b24775318",
    ),
    "zip": (
        "zip — MIT license",
        "Typeshed\n--------",
        "b956e61bb88ab7e9b74c83188ac3616b0bcfb47dea06daaceceba091fdbaca77",
    ),
}


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _require_text(haystack: str, needle: str, label: str) -> None:
    if needle not in haystack:
        raise ValueError(f"{label} is missing pinned identity {needle}")


def _normalized_section(text: str, start: str, end: str) -> str:
    _, separator, remainder = text.partition(f"{start}\n\n")
    if not separator:
        raise ValueError(f"THIRD-PARTY-LICENSES is missing section {start}")
    section, separator, _ = remainder.partition(f"\n\n{end}\n")
    if not separator:
        raise ValueError(f"THIRD-PARTY-LICENSES section {start} has no end marker")
    return "".join(section.split())


def _verify_typeshed_policy_metadata(repo_root: Path) -> None:
    """Bind release notices to the exact snapshot and acquisition runtime."""

    crate_root = repo_root / "crates" / "basilisk-stubs"
    manifest = json.loads(
        (crate_root / "data" / "typeshed" / "manifest.json").read_text()
    )
    bundle_path = crate_root / "data" / "typeshed" / "stdlib.zip"
    bundle = bundle_path.read_bytes()
    if _sha256(bundle) != manifest["bundle"]["sha256"]:
        raise ValueError("embedded Typeshed ZIP differs from its sidecar SHA-256")

    third_party = (repo_root / "THIRD-PARTY-LICENSES").read_bytes()
    with zipfile.ZipFile(bundle_path) as archive:
        for legal_file in manifest["license_manifest"]["files"]:
            payload = archive.read(legal_file["path"])
            if _sha256(payload) != legal_file["sha256"]:
                raise ValueError(
                    f"embedded {legal_file['path']} differs from its approved SHA-256"
                )
            if payload not in third_party:
                raise ValueError(
                    f"THIRD-PARTY-LICENSES does not retain exact {legal_file['path']}"
                )

    notices = (repo_root / "NOTICES").read_text()
    identity_tokens = [
        manifest["source"]["commit_sha"],
        manifest["source"]["tree_sha"],
        manifest["bundle"]["sha256"],
        manifest["versions"]["sha256"],
        *(legal_file["sha256"] for legal_file in manifest["license_manifest"]["files"]),
    ]
    for derived_index in manifest["derived_indexes"].values():
        index_path = crate_root / derived_index["path"]
        index_bytes = index_path.read_bytes()
        if _sha256(index_bytes) != derived_index["sha256"]:
            raise ValueError(f"derived index {index_path} differs from its sidecar")
        identity_tokens.append(derived_index["sha256"])
    for token in identity_tokens:
        _require_text(notices, token, "NOTICES")

    lock = tomllib.loads((repo_root / "Cargo.lock").read_text())
    locked = {(package["name"], package["version"]) for package in lock["package"]}
    for package, version in TYPESHED_RUNTIME_LICENSE_PACKAGES.items():
        if (package, version) not in locked:
            raise ValueError(
                f"Typeshed runtime dependency {package} changed from reviewed version {version}"
            )
        _require_text(notices, f"{package} {version}", "NOTICES")
    third_party_text = third_party.decode()
    for package, (
        start,
        end,
        approved_hash,
    ) in TYPESHED_RUNTIME_LICENSE_SECTIONS.items():
        normalized = _normalized_section(third_party_text, start, end).encode()
        if _sha256(normalized) != approved_hash:
            raise ValueError(
                f"THIRD-PARTY-LICENSES ({package}) differs from its approved license section"
            )


def _legal_sources(repo_root: Path) -> dict[str, bytes]:
    return {name: (repo_root / name).read_bytes() for name in LEGAL_FILES}


def _safe_parts(name: str) -> tuple[str, ...]:
    path = PurePosixPath(name)
    if path.is_absolute() or "\\" in name or ".." in path.parts:
        raise ValueError(f"unsafe archive path: {name}")
    return path.parts


def _verify_entries(
    entries: list[tuple[str, int, bytes]],
    expected: dict[str, bytes],
    *,
    wheel: bool,
) -> None:
    parents: set[tuple[str, ...]] = set()
    for legal_name, legal_bytes in expected.items():
        matches = []
        for archive_name, declared_size, payload in entries:
            parts = _safe_parts(archive_name)
            if not parts or parts[-1] != legal_name:
                continue
            if wheel:
                if (
                    len(parts) < 3
                    or parts[-2] != "licenses"
                    or not parts[-3].endswith(".dist-info")
                ):
                    continue
            matches.append((parts, declared_size, payload))
        if len(matches) != 1:
            raise ValueError(
                f"expected exactly one packaged {legal_name}, found {len(matches)}"
            )
        parts, declared_size, payload = matches[0]
        if declared_size != len(legal_bytes) or payload != legal_bytes:
            raise ValueError(f"packaged {legal_name} differs from the repository copy")
        parents.add(parts[:-1])
    if len(parents) != 1:
        raise ValueError("packaged attribution files do not share one directory")


def verify_zip(archive: Path, repo_root: Path, *, wheel: bool) -> None:
    expected = _legal_sources(repo_root)
    entries: list[tuple[str, int, bytes]] = []
    with zipfile.ZipFile(archive) as package:
        for info in package.infolist():
            if info.is_dir() or PurePosixPath(info.filename).name not in LEGAL_FILES:
                continue
            expected_size = len(expected.get(PurePosixPath(info.filename).name, b""))
            if info.file_size != expected_size:
                entries.append((info.filename, info.file_size, b""))
                continue
            entries.append((info.filename, info.file_size, package.read(info)))
    _verify_entries(entries, expected, wheel=wheel)


def verify_tar(archive: Path, repo_root: Path) -> None:
    expected = _legal_sources(repo_root)
    entries: list[tuple[str, int, bytes]] = []
    with tarfile.open(archive, "r:*") as package:
        for member in package.getmembers():
            if PurePosixPath(member.name).name not in LEGAL_FILES:
                continue
            payload = b""
            if member.isfile() and member.size == len(
                expected[PurePosixPath(member.name).name]
            ):
                extracted = package.extractfile(member)
                if extracted is not None:
                    payload = extracted.read()
            entries.append((member.name, member.size, payload))
    _verify_entries(entries, expected, wheel=False)


def verify(archive: Path, repo_root: Path, kind: str) -> None:
    if kind == "wheel" or archive.suffix == ".whl":
        verify_zip(archive, repo_root, wheel=True)
    elif archive.name.endswith((".tar.gz", ".tgz")):
        verify_tar(archive, repo_root)
    else:
        verify_zip(archive, repo_root, wheel=False)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?")
    parser.add_argument("--kind", choices=("binary", "wheel"), default="binary")
    parser.add_argument(
        "--policy-only",
        action="store_true",
        help="verify embedded Typeshed and dependency-license metadata only",
    )
    parser.add_argument(
        "--repo-root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    args = parser.parse_args()
    _verify_typeshed_policy_metadata(args.repo_root)
    if args.policy_only:
        print("Typeshed release policy metadata verified")
        return
    if args.archive is None:
        parser.error("archive is required unless --policy-only is used")
    verify(args.archive, args.repo_root, args.kind)
    print(f"{args.archive}: exact attribution files verified")


if __name__ == "__main__":
    main()
