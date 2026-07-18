#!/usr/bin/env python3
"""Build Basilisk's exact, deterministic stdlib Typeshed bundle.

The updater never invokes Git. It resolves one full commit through GitHub's API,
checks every codeload file against the API's Git blob identity, applies the
build-approved legal-file manifest, and writes a stored ZIP plus sidecar.

Implements [STUBRES-TYPESHED-BASELINE] and [STUBRES-TYPESHED-LICENSE].
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import tempfile
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any


DEFAULT_COMMIT = "83c2518a9e6abbda0c44592c3483de459198f887"
REPOSITORY = "https://github.com/python/typeshed"
API_ROOT = "https://api.github.com/repos/python/typeshed"
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_EXPANDED_BYTES = 256 * 1024 * 1024
MAX_ENTRY_BYTES = 16 * 1024 * 1024
MAX_ENTRIES = 20_000
MAX_COMPRESSION_RATIO = 100
FIXED_TIMESTAMP = (1980, 1, 1, 0, 0, 0)
APPROVED_LICENSE_MANIFEST = {
    "version": 1,
    "files": [
        {
            "path": "LICENSE",
            "sha256": "295f8538c94ae5c3043301cf7cff1c852dab6a786a8ddee471e061b40d5ecabe",
        }
    ],
}


def _download(url: str, limit: int) -> bytes:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "Basilisk-typeshed-bundle-updater",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        if response.url.startswith("https://") is False:
            raise ValueError(f"refused non-HTTPS response: {response.url}")
        payload = response.read(limit + 1)
    if len(payload) > limit:
        raise ValueError(f"download exceeds {limit} bytes: {url}")
    return payload


def _download_json(url: str) -> dict[str, Any]:
    value = json.loads(_download(url, MAX_ARCHIVE_BYTES))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object from {url}")
    return value


def _git_blob_sha(payload: bytes) -> str:
    header = f"blob {len(payload)}\0".encode()
    return hashlib.sha1(header + payload, usedforsecurity=False).hexdigest()


def _safe_relative_path(name: str) -> str:
    path = PurePosixPath(name)
    if path.is_absolute() or "\\" in name or ".." in path.parts or "\x00" in name:
        raise ValueError(f"unsafe archive path: {name}")
    normalized = path.as_posix()
    if normalized in ("", "."):
        raise ValueError(f"empty archive path: {name}")
    return normalized


def _expected_blobs(tree: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if tree.get("truncated") is not False:
        raise ValueError("GitHub returned a truncated Typeshed tree")
    result: dict[str, dict[str, Any]] = {}
    for entry in tree.get("tree", []):
        entry_type = entry.get("type")
        if entry_type == "commit":
            raise ValueError(f"submodule is not supported: {entry.get('path')}")
        if entry_type != "blob":
            continue
        path = _safe_relative_path(str(entry["path"]))
        mode = str(entry.get("mode", ""))
        if mode not in ("100644", "100755"):
            raise ValueError(f"unsupported Git mode {mode} for {path}")
        if path in result:
            raise ValueError(f"duplicate Git path: {path}")
        result[path] = entry
    return result


def _archive_files(archive: bytes) -> dict[str, bytes]:
    result: dict[str, bytes] = {}
    expanded = 0
    with zipfile.ZipFile(io.BytesIO(archive)) as source:
        infos = source.infolist()
        if len(infos) > MAX_ENTRIES:
            raise ValueError(f"archive contains more than {MAX_ENTRIES} entries")
        roots = {
            PurePosixPath(_safe_relative_path(info.filename)).parts[0]
            for info in infos
            if info.filename
        }
        if len(roots) != 1:
            raise ValueError("codeload archive must have exactly one root directory")
        root = next(iter(roots))
        for info in infos:
            normalized = _safe_relative_path(info.filename)
            parts = PurePosixPath(normalized).parts
            if not parts or parts[0] != root:
                raise ValueError(f"archive entry escaped common root: {normalized}")
            if info.is_dir():
                continue
            if info.flag_bits & 0x1:
                raise ValueError(f"encrypted archive entry is forbidden: {normalized}")
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ValueError(f"unsupported ZIP compression for {normalized}")
            if info.file_size > MAX_ENTRY_BYTES:
                raise ValueError(f"archive entry is too large: {normalized}")
            if info.file_size > max(1, info.compress_size) * MAX_COMPRESSION_RATIO:
                raise ValueError(
                    f"archive entry exceeds compression-ratio limit: {normalized}"
                )
            expanded += info.file_size
            if expanded > MAX_EXPANDED_BYTES:
                raise ValueError("archive exceeds expanded-size limit")
            relative = PurePosixPath(*parts[1:]).as_posix()
            if relative in ("", ".") or relative in result:
                raise ValueError(f"duplicate or empty archive path: {relative}")
            result[relative] = source.read(info)
    return result


def _verify_source(
    files: dict[str, bytes], expected: dict[str, dict[str, Any]]
) -> None:
    missing = sorted(set(expected) - set(files))
    extra = sorted(set(files) - set(expected))
    if missing or extra:
        raise ValueError(
            f"codeload tree differs from GitHub metadata; missing={missing[:3]}, extra={extra[:3]}"
        )
    for path, payload in files.items():
        metadata = expected[path]
        if metadata.get("size") != len(payload):
            raise ValueError(f"Git size mismatch: {path}")
        if metadata.get("sha") != _git_blob_sha(payload):
            raise ValueError(f"Git blob mismatch: {path}")


def _is_relevant_legal_file(path: str) -> bool:
    parts = PurePosixPath(path).parts
    if not parts:
        return False
    basename = parts[-1].upper()
    legal_name = basename.startswith("LICENSE") or basename.startswith("NOTICE")
    return legal_name and (len(parts) == 1 or parts[0] == "stdlib")


def _license_manifest(files: dict[str, bytes]) -> dict[str, Any]:
    discovered = {
        "version": 1,
        "files": [
            {"path": path, "sha256": hashlib.sha256(files[path]).hexdigest()}
            for path in sorted(files)
            if _is_relevant_legal_file(path)
        ],
    }
    if discovered != APPROVED_LICENSE_MANIFEST:
        raise ValueError(
            "Typeshed license/NOTICE identity changed; human review and an explicit "
            "APPROVED_LICENSE_MANIFEST update are required"
        )
    return discovered


def _selected_files(files: dict[str, bytes]) -> dict[str, bytes]:
    selected = {
        path: payload
        for path, payload in files.items()
        if path == "stdlib/VERSIONS"
        or (path.startswith("stdlib/") and path.endswith(".pyi"))
        or _is_relevant_legal_file(path)
    }
    if "stdlib/VERSIONS" not in selected or "LICENSE" not in selected:
        raise ValueError("Typeshed source is missing stdlib/VERSIONS or root LICENSE")
    if not any(path.endswith(".pyi") for path in selected):
        raise ValueError("Typeshed source contains no stdlib .pyi files")
    return selected


def _distribution_map(files: dict[str, bytes]) -> dict[str, str]:
    candidates: dict[str, set[str]] = {}
    for path in sorted(files):
        parts = PurePosixPath(path).parts
        if (
            len(parts) < 3
            or parts[0] != "stubs"
            or parts[2].startswith("@")
            or not path.endswith(".pyi")
        ):
            continue
        distribution = f"types-{parts[1]}"
        import_root = parts[2][:-4] if parts[2].endswith(".pyi") else parts[2]
        candidates.setdefault(import_root, set()).add(distribution)
    # The runtime lookup currently keys on the first import segment. Namespace
    # roots such as `google` can be supplied by several distributions, so no
    # single install suggestion is correct; omit those roots rather than guess.
    distributions = {
        import_root: next(iter(matches))
        for import_root, matches in candidates.items()
        if len(matches) == 1
    }
    if not distributions:
        raise ValueError("Typeshed source contains no third-party stub distributions")
    return distributions


def _distribution_tsv(
    distributions: dict[str, str], commit: str, tree_sha: str
) -> bytes:
    header = [
        "# typeshed third-party stub distribution map — GENERATED, do not edit by hand.",
        "# Implements [STUBRES-TYPESHED-BASELINE].",
        f"# Source commit: python/typeshed@{commit}",
        f"# Source root tree: {tree_sha}",
        "# Format: <import_root>\\t<distribution>",
    ]
    rows = [
        f"{import_root}\t{distributions[import_root]}"
        for import_root in sorted(distributions)
    ]
    return ("\n".join([*header, *rows]) + "\n").encode()


def _zip_bytes(files: dict[str, bytes]) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as bundle:
        for path in sorted(files):
            info = zipfile.ZipInfo(path, date_time=FIXED_TIMESTAMP)
            info.compress_type = zipfile.ZIP_STORED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            bundle.writestr(info, files[path])
    return output.getvalue()


def _atomic_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            os.fchmod(output.fileno(), 0o644)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def _canonical_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def build(
    commit: str,
    output: Path,
    manifest_path: Path,
    distributions_path: Path,
) -> dict[str, Any]:
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise ValueError(
            "bundle updates require one explicit full lowercase commit SHA"
        )
    commit_metadata = _download_json(f"{API_ROOT}/commits/{commit}")
    resolved_commit = str(commit_metadata.get("sha", ""))
    if resolved_commit != commit:
        raise ValueError(
            f"GitHub resolved {commit} to unexpected commit {resolved_commit}"
        )
    tree_sha = str(commit_metadata["commit"]["tree"]["sha"])
    tree = _download_json(f"{API_ROOT}/git/trees/{tree_sha}?recursive=1")
    expected = _expected_blobs(tree)
    source_archive = _download(
        f"https://codeload.github.com/python/typeshed/zip/{commit}", MAX_ARCHIVE_BYTES
    )
    source_files = _archive_files(source_archive)
    _verify_source(source_files, expected)
    license_manifest = _license_manifest(source_files)
    selected = _selected_files(source_files)
    distributions = _distribution_map(source_files)
    distribution_tsv = _distribution_tsv(distributions, commit, tree_sha)
    bundle = _zip_bytes(selected)
    pyi_count = sum(path.endswith(".pyi") for path in selected)
    legal_identity = hashlib.sha256(_canonical_json(license_manifest)).hexdigest()
    manifest = {
        "bundle": {
            "file_count": len(selected),
            "format": "zip-stored",
            "pyi_count": pyi_count,
            "sha256": hashlib.sha256(bundle).hexdigest(),
            "scope": "stdlib-subset",
            "uncompressed_bytes": sum(map(len, selected.values())),
        },
        "license_manifest": {**license_manifest, "identity_sha256": legal_identity},
        "derived_indexes": {
            "stub_distributions": {
                "entries": len(distributions),
                "path": "data/typeshed_stub_distributions.tsv",
                "sha256": hashlib.sha256(distribution_tsv).hexdigest(),
            }
        },
        "schema_version": 1,
        "source": {
            "commit_sha": commit,
            "repository": REPOSITORY,
            "tree_sha": tree_sha,
            "tree_scope": "full-repository",
            "verification": {
                "method": "github-api-tree-and-full-codeload-blob-set-match",
                "signed_release": False,
                "trust_boundary": "github-tls",
            },
        },
        "versions": {
            "path": "stdlib/VERSIONS",
            "sha256": hashlib.sha256(selected["stdlib/VERSIONS"]).hexdigest(),
        },
    }
    _atomic_write(output, bundle)
    _atomic_write(manifest_path, _canonical_json(manifest))
    _atomic_write(distributions_path, distribution_tsv)
    return manifest


def main() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    default_dir = repo_root / "crates" / "basilisk-stubs" / "data" / "typeshed"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", default=DEFAULT_COMMIT)
    parser.add_argument("--output", type=Path, default=default_dir / "stdlib.zip")
    parser.add_argument("--manifest", type=Path, default=default_dir / "manifest.json")
    parser.add_argument(
        "--distributions",
        type=Path,
        default=repo_root
        / "crates"
        / "basilisk-stubs"
        / "data"
        / "typeshed_stub_distributions.tsv",
    )
    args = parser.parse_args()
    manifest = build(args.commit, args.output, args.manifest, args.distributions)
    print(
        f"wrote {args.output}: {manifest['bundle']['file_count']} files, "
        f"sha256={manifest['bundle']['sha256']}"
    )


if __name__ == "__main__":
    main()
