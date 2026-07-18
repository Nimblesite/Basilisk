"""Tests for the release legal-file gate ([STUBRES-TYPESHED-LICENSE])."""

from __future__ import annotations

import importlib.util
import io
import json
import shutil
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


SCRIPT = (
    Path(__file__).resolve().parents[1] / "scripts" / "verify_release_attribution.py"
)
REPO_ROOT = SCRIPT.parents[1]
SPEC = importlib.util.spec_from_file_location("verify_release_attribution", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReleaseAttributionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        for name in MODULE.LEGAL_FILES:
            (self.root / name).write_bytes(f"exact {name}\n".encode())

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _zip(self, name: str, prefix: str = "") -> Path:
        archive = self.root / name
        with zipfile.ZipFile(archive, "w") as package:
            for legal_name in MODULE.LEGAL_FILES:
                package.writestr(
                    f"{prefix}{legal_name}", (self.root / legal_name).read_bytes()
                )
        return archive

    def test_binary_zip_accepts_one_common_package_root(self) -> None:
        MODULE.verify(self._zip("release.zip", "basilisk/"), self.root, "binary")

    def test_wheel_accepts_dist_info_license_directory(self) -> None:
        wheel = self._zip("release.whl", "basilisk-1.0.dist-info/licenses/")
        MODULE.verify(wheel, self.root, "wheel")

    def test_tar_accepts_exact_legal_files(self) -> None:
        archive = self.root / "release.tar.gz"
        with tarfile.open(archive, "w:gz") as package:
            for legal_name in MODULE.LEGAL_FILES:
                payload = (self.root / legal_name).read_bytes()
                info = tarfile.TarInfo(legal_name)
                info.size = len(payload)
                package.addfile(info, io.BytesIO(payload))
        MODULE.verify(archive, self.root, "binary")

    def test_rejects_missing_attribution(self) -> None:
        archive = self._zip("missing.zip")
        with zipfile.ZipFile(archive, "w") as package:
            package.writestr("LICENSE", (self.root / "LICENSE").read_bytes())
        with self.assertRaisesRegex(ValueError, "NOTICES"):
            MODULE.verify(archive, self.root, "binary")

    def test_rejects_stale_attribution(self) -> None:
        archive = self.root / "stale.zip"
        with zipfile.ZipFile(archive, "w") as package:
            for legal_name in MODULE.LEGAL_FILES:
                payload = (
                    b"stale"
                    if legal_name == "NOTICES"
                    else (self.root / legal_name).read_bytes()
                )
                package.writestr(legal_name, payload)
        with self.assertRaisesRegex(ValueError, "NOTICES"):
            MODULE.verify(archive, self.root, "binary")

    def test_typeshed_runtime_license_gate_covers_archive_and_tls_dependencies(
        self,
    ) -> None:
        self.assertEqual(
            MODULE.TYPESHED_RUNTIME_LICENSE_PACKAGES["zip"],
            "5.1.1",
        )
        self.assertEqual(
            MODULE.TYPESHED_RUNTIME_LICENSE_PACKAGES["subtle"],
            "2.6.1",
        )

    def test_policy_rejects_truncated_runtime_license_sections(self) -> None:
        # [STUBRES-TYPESHED-LICENSE] Keeping only a copyright sentinel is not
        # enough: binary redistribution conditions and disclaimers must remain.
        truncations = {
            "subtle": (
                "subtle — BSD-3-Clause license",
                "2. Redistributions in binary form must reproduce the above copyright notice,\n"
                "this list of conditions and the following disclaimer in the documentation\n"
                "and/or other materials provided with the distribution.",
                "2. [truncated]",
            ),
            "zip": (
                "zip — MIT license",
                "The above copyright notice and this permission notice shall be included in all\n"
                "copies or substantial portions of the Software.",
                "[truncated]",
            ),
        }
        for package, (heading, complete, truncated) in truncations.items():
            with self.subTest(package=package), tempfile.TemporaryDirectory() as temp:
                policy_root = Path(temp)
                for name in (*MODULE.LEGAL_FILES, "Cargo.lock"):
                    shutil.copy2(REPO_ROOT / name, policy_root / name)
                crate_root = policy_root / "crates" / "basilisk-stubs"
                crate_root.mkdir(parents=True)
                shutil.copytree(
                    REPO_ROOT / "crates" / "basilisk-stubs" / "data",
                    crate_root / "data",
                )
                licenses = policy_root / "THIRD-PARTY-LICENSES"
                text = licenses.read_text()
                before, separator, section_and_remainder = text.partition(heading)
                self.assertTrue(separator)
                self.assertIn(complete, section_and_remainder)
                licenses.write_text(
                    before
                    + separator
                    + section_and_remainder.replace(complete, truncated, 1)
                )

                with self.assertRaises(ValueError):
                    MODULE._verify_typeshed_policy_metadata(policy_root)

    def test_direct_vscode_package_routes_through_verified_release_recipe(self) -> None:
        # [VSIX-PACKAGING-PARITY] The public npm entry point must not invoke
        # `vsce package` directly and bypass the final VSIX verifier.
        package = json.loads(
            (REPO_ROOT / "vscode-extension" / "package.json").read_text()
        )
        script = package["scripts"]["package"]
        self.assertIn("make _release_vsix", script)
        self.assertNotIn("vsce package", script)

    def test_readmes_describe_typeshed_composite_license(self) -> None:
        # [STUBRES-TYPESHED-LICENSE] Typeshed is not Apache-only: its root
        # LICENSE says parts of the project use other licenses including MIT.
        for relative in (
            "README.md",
            "README-pypi.md",
            "vscode-extension/README.md",
        ):
            with self.subTest(readme=relative):
                self.assertIn(
                    "Apache-2.0, with MIT-licensed parts",
                    (REPO_ROOT / relative).read_text(),
                )
        for relative in ("README.zh.md", "vscode-extension/README.zh.md"):
            with self.subTest(readme=relative):
                self.assertIn(
                    "Apache-2.0，部分内容采用 MIT 许可证",
                    (REPO_ROOT / relative).read_text(),
                )


if __name__ == "__main__":
    unittest.main()
