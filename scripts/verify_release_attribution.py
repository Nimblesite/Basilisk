#!/usr/bin/env python3
"""Verify exact legal files in shipped archives ([STUBRES-TYPESHED-LICENSE])."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tarfile
import tomllib
import zipfile
from pathlib import Path, PurePosixPath


LEGAL_FILES = (
    "LICENSE",
    "NOTICES",
    "THIRD-PARTY-LICENSES",
    "RUST-DEPENDENCY-LICENSES",
)
TYPESHED_RUNTIME_LICENSE_PACKAGES = {
    "ring": "0.17.14",
    "rustls": "0.23.43",
    "rustls-pki-types": "1.15.1",
    "rustls-webpki": "0.103.13",
    "subtle": "2.6.1",
    "untrusted": "0.9.0",
    "ureq": "3.3.0",
    "ureq-proto": "0.6.0",
    "utf8-zero": "0.8.1",
    "webpki-roots": "1.0.9",
    "zip": "8.6.0",
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


def _cargo_dependency_graph_sha256(path: Path) -> str:
    lock = tomllib.loads(path.read_text())
    packages = [package for package in lock["package"] if "source" in package]
    packages.sort(
        key=lambda package: (package["name"], package["version"], package["source"])
    )
    payload = json.dumps(packages, sort_keys=True, separators=(",", ":")).encode()
    return _sha256(payload)


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


# Byte-compared by the VSIX pipeline: vscode-extension/scripts/
# update-dependency-licenses.mjs --check regenerates the carrier + manifest and
# requires the committed bytes to match exactly, and the release workflow packs
# VSCODE-DISTRIBUTION-LICENSE verbatim into the VSIX as LICENSE.txt.
VSIX_LEGAL_FILES = (
    "VSCODE-DISTRIBUTION-LICENSE",
    "VSCODE-DEPENDENCY-LICENSES",
    "vscode-license-manifest.json",
)


def _byte_exact_repo_paths(repo_root: Path) -> tuple[str, ...]:
    """Working-tree files whose exact bytes the release pipeline asserts."""
    crate_root = PurePosixPath("crates/basilisk-stubs")
    manifest = json.loads(
        (repo_root / crate_root / "data" / "typeshed" / "manifest.json").read_text()
    )
    derived = tuple(
        str(crate_root / index["path"])
        for index in manifest["derived_indexes"].values()
    )
    return (
        *LEGAL_FILES,
        *VSIX_LEGAL_FILES,
        str(crate_root / "data" / "typeshed" / "stdlib.zip"),
        *derived,
    )


def _verify_checkout_byte_stability(repo_root: Path) -> None:
    """Every byte-verified file must be pinned against checkout rewriting.

    Git-for-Windows defaults to core.autocrlf=true, so a byte-verified file
    left eligible for text conversion checks out with CRLF endings on the
    windows-latest runners and its bytes no longer match the recorded
    sidecar/notice (the v0.36.0/v0.37.0 release failures). `text` must
    resolve to `unset` for these paths (`-text` or `binary` in .gitattributes).
    """
    paths = _byte_exact_repo_paths(repo_root)
    result = subprocess.run(
        ["git", "-C", str(repo_root), "check-attr", "text", "--", *paths],
        check=True,
        capture_output=True,
        text=True,
    )
    attributes = dict(line.rsplit(": text: ", 1) for line in result.stdout.splitlines())
    for path in paths:
        if attributes.get(path) != "unset":
            raise ValueError(
                f"{path} is byte-verified but not pinned '-text' in .gitattributes "
                f"(text={attributes.get(path)!r}); a core.autocrlf=true checkout "
                "would rewrite its bytes and break attribution verification"
            )


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


def _verify_release_package_metadata(repo_root: Path) -> None:
    runtime_manifest = json.loads(
        (repo_root / "runtime-license-manifest.json").read_text()
    )
    if (
        _cargo_dependency_graph_sha256(repo_root / "Cargo.lock")
        != runtime_manifest["cargo_dependency_graph_sha256"]
    ):
        raise ValueError(
            "locked third-party Cargo graph changed without regenerating runtime licenses"
        )
    runtime_licenses = (repo_root / "RUST-DEPENDENCY-LICENSES").read_bytes()
    if _sha256(runtime_licenses) != runtime_manifest["licenses_sha256"]:
        raise ValueError("RUST-DEPENDENCY-LICENSES differs from its manifest")
    runtime_text = runtime_licenses.decode()
    if "basilisk-cli 0.0.0-PLACEHOLDER" in runtime_text:
        raise ValueError(
            "first-party workspace crates leaked into the dependency carrier"
        )
    # A spot-check that the carrier really carries license TEXT, not just a
    # crate list. The set is small because the shipped graph is small: the
    # binary is inert ([WITHDRAWAL-INERT]) and links Shipwright and its serde
    # stack, nothing else. The crates that used to appear here — the typeshed
    # download runtime, the embedded formatter, their transitive graph — are not
    # linked in any more, and listing licenses for code that does not ship would
    # be a claim about the binary that is not true. The exact carrier is still
    # pinned by `licenses_sha256` above; this only proves it is not a stub.
    for required_notice in (
        "Apache License",
        "MIT License",
        "UNICODE LICENSE V3",
    ):
        _require_text(runtime_text, required_notice, "RUST-DEPENDENCY-LICENSES")

    pyproject = tomllib.loads((repo_root / "pyproject.toml").read_text())
    expressions = set(runtime_manifest["wheel_license_expressions"].values())
    if pyproject["project"]["license"] not in expressions:
        raise ValueError(
            "pyproject.toml License-Expression does not cover the shipped binary"
        )
    extension = json.loads((repo_root / "vscode-extension/package.json").read_text())
    if extension["license"] != "SEE LICENSE IN LICENSE.txt":
        raise ValueError("VSIX manifest does not reference its packaged LICENSE.txt")


def _legal_sources(repo_root: Path) -> dict[str, bytes]:
    return {name: (repo_root / name).read_bytes() for name in LEGAL_FILES}


def _safe_parts(name: str) -> tuple[str, ...]:
    path = PurePosixPath(name)
    if path.is_absolute() or "\\" in name or ".." in path.parts:
        raise ValueError(f"unsafe archive path: {name}")
    return path.parts


def _verify_wheel_license_expression(
    package: zipfile.ZipFile, expected_expression: str
) -> None:
    metadata = [
        info
        for info in package.infolist()
        if not info.is_dir()
        and len(_safe_parts(info.filename)) >= 2
        and _safe_parts(info.filename)[-1] == "METADATA"
        and _safe_parts(info.filename)[-2].endswith(".dist-info")
    ]
    if len(metadata) != 1:
        raise ValueError(f"expected one wheel METADATA file, found {len(metadata)}")
    text = package.read(metadata[0]).decode("utf-8")
    expressions = [
        line.partition(":")[2].strip()
        for line in text.splitlines()
        if line.startswith("License-Expression:")
    ]
    if expressions != [expected_expression]:
        raise ValueError("wheel License-Expression does not cover the shipped binary")
    license_files = [
        line.partition(":")[2].strip()
        for line in text.splitlines()
        if line.startswith("License-File:")
    ]
    if len(license_files) != len(LEGAL_FILES) or set(license_files) != set(LEGAL_FILES):
        raise ValueError("wheel METADATA does not name every packaged License-File")


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


def verify_zip(
    archive: Path,
    repo_root: Path,
    *,
    wheel: bool,
    wheel_license_expression: str | None,
) -> None:
    expected = _legal_sources(repo_root)
    entries: list[tuple[str, int, bytes]] = []
    with zipfile.ZipFile(archive) as package:
        if wheel:
            if wheel_license_expression is None:
                raise ValueError("wheel verification requires a license expression")
            _verify_wheel_license_expression(package, wheel_license_expression)
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


def verify(
    archive: Path, repo_root: Path, kind: str, *, target: str | None = None
) -> None:
    if kind == "wheel" or archive.suffix == ".whl":
        if target is None:
            raise ValueError("wheel verification requires an exact release target")
        manifest = json.loads((repo_root / "runtime-license-manifest.json").read_text())
        try:
            expected = manifest["wheel_license_expressions"][target]
        except KeyError as error:
            raise ValueError(f"unsupported wheel target: {target}") from error
        verify_zip(
            archive,
            repo_root,
            wheel=True,
            wheel_license_expression=expected,
        )
    elif archive.name.endswith((".tar.gz", ".tgz")):
        verify_tar(archive, repo_root)
    else:
        verify_zip(archive, repo_root, wheel=False, wheel_license_expression=None)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?")
    parser.add_argument("--kind", choices=("binary", "wheel"), default="binary")
    parser.add_argument("--target")
    parser.add_argument(
        "--policy-only",
        action="store_true",
        help="verify embedded Typeshed and dependency-license metadata only",
    )
    parser.add_argument(
        "--repo-root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    args = parser.parse_args()
    _verify_checkout_byte_stability(args.repo_root)
    _verify_typeshed_policy_metadata(args.repo_root)
    _verify_release_package_metadata(args.repo_root)
    if args.policy_only:
        print("Release attribution policy metadata verified")
        return
    if args.archive is None:
        parser.error("archive is required unless --policy-only is used")
    verify(args.archive, args.repo_root, args.kind, target=args.target)
    print(f"{args.archive}: exact attribution files verified")


if __name__ == "__main__":
    main()
