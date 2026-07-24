"""Offline bundle integrity tests ([STUBRES-TYPESHED-BASELINE])."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import unittest
import zipfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "crates" / "basilisk-stubs" / "data" / "typeshed" / "stdlib.zip"
MANIFEST = BUNDLE.with_name("manifest.json")
STUB_DISTRIBUTIONS = BUNDLE.parents[1] / "typeshed_stub_distributions.tsv"
UPDATER = ROOT / "scripts" / "update_typeshed_bundle.py"
SPEC = importlib.util.spec_from_file_location("update_typeshed_bundle", UPDATER)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class TypeshedBundleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = json.loads(MANIFEST.read_text())
        cls.bundle_bytes = BUNDLE.read_bytes()
        with zipfile.ZipFile(BUNDLE) as bundle:
            cls.infos = bundle.infolist()
            cls.files = {info.filename: bundle.read(info) for info in cls.infos}

    def test_exact_pinned_source_identity(self) -> None:
        source = self.manifest["source"]
        self.assertEqual(
            source["commit_sha"], "83c2518a9e6abbda0c44592c3483de459198f887"
        )
        self.assertEqual(source["repository"], "https://github.com/python/typeshed")
        self.assertEqual(source["tree_sha"], "66408ffce2750980efc6da09e8a6652733f852e4")
        self.assertEqual(source["tree_scope"], "full-repository")
        self.assertEqual(
            source["verification"],
            {
                "method": "github-api-tree-and-full-codeload-blob-set-match",
                "signed_release": False,
                "trust_boundary": "github-tls",
            },
        )

    def test_exact_complete_stdlib_selection(self) -> None:
        names = set(self.files)
        pyi = {name for name in names if name.endswith(".pyi")}
        self.assertEqual(len(pyi), 752)
        self.assertTrue(all(name.startswith("stdlib/") for name in pyi))
        self.assertEqual(names - pyi, {"LICENSE", "stdlib/VERSIONS"})
        self.assertEqual(len(names), 754)
        self.assertFalse(any(name.startswith("stdlib/@tests/") for name in names))
        self.assertFalse(any(name.startswith("stubs/") for name in names))

    def test_bundle_and_versions_digests(self) -> None:
        bundle = self.manifest["bundle"]
        self.assertEqual(
            hashlib.sha256(self.bundle_bytes).hexdigest(), bundle["sha256"]
        )
        self.assertEqual(bundle["format"], "zip-stored")
        self.assertEqual(bundle["scope"], "stdlib-subset")
        self.assertEqual(bundle["file_count"], 754)
        self.assertEqual(bundle["pyi_count"], 752)
        self.assertEqual(bundle["uncompressed_bytes"], 2_852_844)
        self.assertEqual(
            hashlib.sha256(self.files["stdlib/VERSIONS"]).hexdigest(),
            "8a236d098757a04bdaeb40bb78545d0f9becc8db6a834c21b3d86e8d4d82bce3",
        )

    def test_distribution_index_is_identity_bound_and_deterministic(self) -> None:
        payload = STUB_DISTRIBUTIONS.read_bytes()
        metadata = self.manifest["derived_indexes"]["stub_distributions"]
        self.assertEqual(metadata["path"], "data/typeshed_stub_distributions.tsv")
        self.assertEqual(metadata["entries"], 274)
        self.assertEqual(
            metadata["sha256"],
            "dc995d1599210db5b5f472d6eb6869130e229c1c9a0d3a4bf158e28db25c5639",
        )
        self.assertEqual(hashlib.sha256(payload).hexdigest(), metadata["sha256"])
        lines = payload.decode().splitlines()
        self.assertIn(
            "# Source commit: python/typeshed@83c2518a9e6abbda0c44592c3483de459198f887",
            lines,
        )
        self.assertIn(
            "# Source root tree: 66408ffce2750980efc6da09e8a6652733f852e4", lines
        )
        rows = [line for line in lines if line and not line.startswith("#")]
        self.assertEqual(len(rows), metadata["entries"])
        self.assertEqual(rows, sorted(set(rows)))
        self.assertIn("requests\ttypes-requests", rows)
        self.assertIn("yaml\ttypes-PyYAML", rows)

    def test_exact_composite_license_and_no_notice(self) -> None:
        license_manifest = self.manifest["license_manifest"]
        approved = {
            "version": license_manifest["version"],
            "files": license_manifest["files"],
        }
        self.assertEqual(approved, MODULE.APPROVED_LICENSE_MANIFEST)
        self.assertEqual(
            hashlib.sha256(MODULE._canonical_json(approved)).hexdigest(),
            license_manifest["identity_sha256"],
        )
        self.assertEqual(
            hashlib.sha256(self.files["LICENSE"]).hexdigest(),
            "295f8538c94ae5c3043301cf7cff1c852dab6a786a8ddee471e061b40d5ecabe",
        )
        legal_names = {
            name
            for name in self.files
            if PurePosixPath(name).name.upper().startswith(("LICENSE", "NOTICE"))
        }
        self.assertEqual(legal_names, {"LICENSE"})
        third_party = (ROOT / "THIRD-PARTY-LICENSES").read_bytes()
        marker = (
            b'The "typeshed" project is licensed under the terms of the Apache license'
        )
        self.assertEqual(
            third_party[third_party.index(marker) :], self.files["LICENSE"]
        )

    def test_zip_is_deterministic_and_normalized(self) -> None:
        self.assertEqual(len(self.infos), len(self.files))
        self.assertEqual([info.filename for info in self.infos], sorted(self.files))
        for info in self.infos:
            self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
            self.assertEqual(info.date_time, MODULE.FIXED_TIMESTAMP)
            path = PurePosixPath(info.filename)
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
        self.assertEqual(MODULE._zip_bytes(self.files), self.bundle_bytes)

    def test_updater_rejects_extreme_compression_ratio(self) -> None:
        archive = io.BytesIO()
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as package:
            package.writestr("typeshed-pinned/stdlib/bomb.pyi", b"0" * 1_000_000)
        with self.assertRaisesRegex(ValueError, "compression-ratio"):
            MODULE._archive_files(archive.getvalue())


if __name__ == "__main__":
    unittest.main()
