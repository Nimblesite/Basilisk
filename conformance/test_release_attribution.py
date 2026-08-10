"""Tests for the release legal-file gate ([STUBRES-TYPESHED-LICENSE])."""

from __future__ import annotations

import importlib.util
import io
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
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
        self.wheel_license_expression = "Apache-2.0 AND MIT"
        (self.root / "runtime-license-manifest.json").write_text(
            json.dumps(
                {
                    "wheel_license_expressions": {
                        "test-target": self.wheel_license_expression
                    }
                }
            )
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _zip(
        self,
        name: str,
        prefix: str = "",
        *,
        wheel_license: str | None = None,
        include_license_files: bool = True,
    ) -> Path:
        archive = self.root / name
        with zipfile.ZipFile(archive, "w") as package:
            for legal_name in MODULE.LEGAL_FILES:
                package.writestr(
                    f"{prefix}{legal_name}", (self.root / legal_name).read_bytes()
                )
            if name.endswith(".whl"):
                expression = wheel_license or self.wheel_license_expression
                package.writestr(
                    "basilisk-1.0.dist-info/METADATA",
                    "Metadata-Version: 2.4\n"
                    "Name: basilisk-python\n"
                    f"License-Expression: {expression}\n"
                    + (
                        "".join(
                            f"License-File: {legal_name}\n"
                            for legal_name in MODULE.LEGAL_FILES
                        )
                        if include_license_files
                        else ""
                    ),
                )
        return archive

    def test_binary_zip_accepts_one_common_package_root(self) -> None:
        MODULE.verify(self._zip("release.zip", "basilisk/"), self.root, "binary")

    def test_wheel_accepts_dist_info_license_directory(self) -> None:
        wheel = self._zip("release.whl", "basilisk-1.0.dist-info/licenses/")
        MODULE.verify(wheel, self.root, "wheel", target="test-target")

    def test_wheel_rejects_mit_only_license_metadata(self) -> None:
        wheel = self._zip(
            "mit-only.whl",
            "basilisk-1.0.dist-info/licenses/",
            wheel_license="MIT",
        )
        with self.assertRaisesRegex(ValueError, "License-Expression"):
            MODULE.verify(wheel, self.root, "wheel", target="test-target")

    def test_wheel_rejects_missing_license_file_metadata(self) -> None:
        wheel = self._zip(
            "missing-metadata.whl",
            "basilisk-1.0.dist-info/licenses/",
            include_license_files=False,
        )
        with self.assertRaisesRegex(ValueError, "License-File"):
            MODULE.verify(wheel, self.root, "wheel", target="test-target")

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
        # The archive (`zip`) and TLS (`subtle`) crates must never fall out of
        # the reviewed set — dropping either would ship their licences
        # unattributed. Assert the reviewed version against Cargo.lock rather
        # than a literal copied out of the script: a duplicated pin rots on the
        # next dependency bump and fails HERE, far from the constant it mirrors.
        locked = {
            package["name"]: package["version"]
            for package in tomllib.loads((REPO_ROOT / "Cargo.lock").read_text())[
                "package"
            ]
        }
        for package in ("zip", "subtle"):
            with self.subTest(package=package):
                self.assertIn(package, MODULE.TYPESHED_RUNTIME_LICENSE_PACKAGES)
                self.assertEqual(
                    MODULE.TYPESHED_RUNTIME_LICENSE_PACKAGES[package],
                    locked[package],
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

    def test_release_vsix_recipe_fails_when_packager_produces_no_vsix(self) -> None:
        # [VSIX-PACKAGING-PARITY] A continued shell comment must not truncate
        # the recipe before packaging and final verification run.
        commands = self.root / "bin"
        commands.mkdir()
        successful_stub = "#!/bin/sh\nexit 0\n"
        for name in ("cargo", "cp", "npm", "npx", "python3"):
            command = commands / name
            command.write_text(successful_stub)
            command.chmod(0o755)
        node = commands / "node"
        node.write_text(
            "#!/bin/sh\n"
            'if [ "$2" = "vsix" ] && [ ! -f "$3" ]; then\n'
            '  echo "missing VSIX: $3" >&2\n'
            "  exit 42\n"
            "fi\n"
            "exit 0\n"
        )
        node.chmod(0o755)
        (self.root / "vscode-extension").mkdir()
        environment = os.environ.copy()
        environment.update(
            {
                "BSK_VSIX_TARGET": "linux-x64",
                "PATH": f"{commands}{os.pathsep}{environment['PATH']}",
            }
        )

        result = subprocess.run(
            [
                "make",
                "--no-print-directory",
                "-f",
                str(REPO_ROOT / "Makefile"),
                "_release_vsix",
            ],
            cwd=self.root,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        # The recipe's own verifier is what refuses ([WITHDRAWAL-SURFACES]):
        # scripts/verify-vsix-inert.sh inspects the packaged zip and cannot
        # inspect a file the packager never wrote.
        self.assertIn("no such VSIX", result.stderr)

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

    def test_package_metadata_names_every_license_in_shipped_binaries(self) -> None:
        # PEP 639 License-Expression covers the containing distribution, so the
        # wheel must name every license in the statically linked runtime, not
        # just Basilisk's own MIT source license. The expression is far shorter
        # than it was: the binary is inert ([WITHDRAWAL-INERT]) and links no
        # typeshed snapshot, no embedded formatter, and no download runtime, so
        # naming their licenses would claim they ship when they do not.
        pyproject = tomllib.loads((REPO_ROOT / "pyproject.toml").read_text())
        manifest = json.loads((REPO_ROOT / "runtime-license-manifest.json").read_text())
        expressions = manifest["wheel_license_expressions"]
        self.assertEqual(
            pyproject["project"]["license"],
            expressions["aarch64-apple-darwin"],
        )
        self.assertEqual(set(manifest["targets"]), set(expressions))
        # Every target is covered, and every expression names Basilisk's own
        # license. An empty or partial expression is the failure to catch here.
        for target, expression in expressions.items():
            with self.subTest(target=target):
                self.assertIn("MIT", expression)

        # VS Code's manifest specification requires a packaged root license to
        # be referenced by filename. `vsce` maps source LICENSE to LICENSE.txt.
        extension = json.loads(
            (REPO_ROOT / "vscode-extension" / "package.json").read_text()
        )
        self.assertEqual(extension["license"], "SEE LICENSE IN LICENSE.txt")

    def test_workspace_version_stamp_does_not_stale_runtime_license_graph(self) -> None:
        lock = self.root / "Cargo.lock"
        lock.write_text(
            "version = 4\n\n"
            '[[package]]\nname = "basilisk-cli"\nversion = "0.0.0-PLACEHOLDER"\n\n'
            '[[package]]\nname = "third-party"\nversion = "1.0.0"\n'
            'source = "registry+https://example.invalid"\nchecksum = "abc"\n'
        )
        original = MODULE._cargo_dependency_graph_sha256(lock)
        lock.write_text(lock.read_text().replace("0.0.0-PLACEHOLDER", "9.8.7"))
        self.assertEqual(MODULE._cargo_dependency_graph_sha256(lock), original)
        lock.write_text(
            lock.read_text().replace('version = "1.0.0"', 'version = "1.0.1"')
        )
        self.assertNotEqual(MODULE._cargo_dependency_graph_sha256(lock), original)


if __name__ == "__main__":
    unittest.main()
