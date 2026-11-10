"""Regression tests for the conformance runner's environment bootstrap."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import run_conformance
import worktree_lock
from run_conformance import _is_current_venv


ROOT = Path(__file__).resolve().parents[1]


class HarnessEnvironmentTests(unittest.TestCase):
    def test_symlinked_interpreter_does_not_impersonate_virtualenv(self) -> None:
        """A shared executable target must not be used as venv identity."""
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            system_prefix = root / "system"
            venv = root / "venv"
            system_prefix.mkdir()
            venv.mkdir()

            self.assertFalse(_is_current_venv(venv, prefix=str(system_prefix)))

    def test_matching_prefix_identifies_current_virtualenv(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            venv = Path(temp) / "venv"
            venv.mkdir()

            self.assertTrue(_is_current_venv(venv, prefix=str(venv)))

    def test_default_suite_destinations_are_unique_and_cleanup_owned(self) -> None:
        """Concurrent standalone runs must never share a predictable checkout."""
        first, first_owner = run_conformance._suite_destination(
            run_conformance.parse_args([]), ROOT
        )
        second, second_owner = run_conformance._suite_destination(
            run_conformance.parse_args([]), ROOT
        )
        self.assertIsNotNone(first_owner)
        self.assertIsNotNone(second_owner)
        self.addCleanup(first_owner.cleanup)
        self.addCleanup(second_owner.cleanup)

        self.assertNotEqual(first, second)
        self.assertEqual(first.name, "typing")
        self.assertEqual(second.name, "typing")
        self.assertTrue(first.parent.is_dir())
        self.assertTrue(second.parent.is_dir())

        first_parent = first.parent
        sibling = first_parent.with_name(f"{first_parent.name}-sibling")
        sibling.mkdir()
        self.addCleanup(shutil.rmtree, sibling, True)
        first_owner.cleanup()
        self.assertFalse(first_parent.exists())
        self.assertTrue(sibling.is_dir())

    def test_reuse_requires_a_caller_owned_suite_directory(self) -> None:
        with self.assertRaisesRegex(
            RuntimeError, "--reuse-clone requires an explicit --suite-dir"
        ):
            run_conformance._suite_destination(
                run_conformance.parse_args(["--reuse-clone"]), ROOT
            )

    def test_generated_suite_directory_cannot_inherit_repository_config(self) -> None:
        with patch.object(tempfile, "tempdir", str(ROOT)):
            with self.assertRaisesRegex(
                RuntimeError, "outside the Basilisk repository"
            ):
                run_conformance._suite_destination(run_conformance.parse_args([]), ROOT)

    def test_fresh_clone_refuses_to_delete_an_existing_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "typing"
            destination.mkdir()
            sentinel = destination / "keep-me"
            sentinel.write_text("preserve")

            with patch.object(run_conformance, "run") as run_mock:
                with self.assertRaisesRegex(RuntimeError, "must not already exist"):
                    run_conformance.clone_suite("main", destination)

            run_mock.assert_not_called()
            self.assertEqual(sentinel.read_text(), "preserve")

    def test_reuse_requires_the_active_run_owner_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "typing"
            destination.mkdir()
            opts = run_conformance.parse_args(
                ["--suite-dir", str(destination), "--reuse-clone"]
            )

            with patch.dict("os.environ", {}, clear=True):
                with patch.object(run_conformance, "clone_suite") as clone_mock:
                    with self.assertRaisesRegex(RuntimeError, "active conformance run"):
                        run_conformance.resolve_suite(opts, destination)
            clone_mock.assert_not_called()

    def test_reuse_accepts_only_the_matching_active_run_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "typing"
            destination.mkdir()
            run_conformance._owner_marker(destination).write_text("current-run")
            opts = run_conformance.parse_args(
                ["--suite-dir", str(destination), "--reuse-clone"]
            )
            expected = (
                destination / "conformance",
                {"sha": "a" * 40, "short": "a" * 7, "date": "2026-07-19"},
            )

            with patch.dict(
                "os.environ",
                {run_conformance.RUN_ID_ENV: "current-run"},
                clear=True,
            ):
                with patch.object(
                    run_conformance, "_suite_paths", return_value=expected
                ):
                    self.assertEqual(
                        run_conformance.resolve_suite(opts, destination), expected
                    )

    def test_rust_gate_owns_one_isolated_clone_for_all_three_passes(self) -> None:
        """Sync, coverage, and release scoring must share one run-owned clone."""
        script = (ROOT / "scripts" / "test-rust.sh").read_text()
        invocations = [
            line.strip()
            for line in script.splitlines()
            if "python3" in line
            and "conformance/run_conformance.py" in line
            and not line.lstrip().startswith("#")
        ]

        self.assertEqual(len(invocations), 3)
        self.assertTrue(
            all('--suite-dir "$TYPING_SUITE_DIR"' in line for line in invocations)
        )
        self.assertNotIn("--reuse-clone", invocations[0])
        self.assertTrue(all("--reuse-clone" in line for line in invocations[1:]))
        self.assertIn("mktemp -d", script)
        self.assertIn("BASILISK_TEST_RUST_LOCK_FD", script)
        self.assertIn("BASILISK_CONFORMANCE_RUN_ID", script)
        self.assertIn("conformance/worktree_lock.py", script)

    def test_direct_runner_respects_the_shared_worktree_lock(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            lock_path = root / "target" / "test-rust.lock"
            environment = os.environ.copy()
            environment.pop(worktree_lock.LOCK_FD_ENV, None)
            environment.pop(worktree_lock.LOCK_OWNER_PID_ENV, None)
            command = self._runner_command(root)
            with worktree_lock.exclusive_worktree_lock(lock_path):
                result = subprocess.run(command, env=environment, check=False)
            self.assertEqual(result.returncode, 75)

    def test_invalid_inherited_lock_descriptor_cannot_bypass_ownership(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            lock_path = root / "target" / "test-rust.lock"
            environment = os.environ.copy()
            environment[worktree_lock.LOCK_FD_ENV] = "999999"
            environment[worktree_lock.LOCK_OWNER_PID_ENV] = str(os.getpid())
            command = self._runner_command(root)
            with worktree_lock.exclusive_worktree_lock(lock_path):
                result = subprocess.run(command, env=environment, check=False)
            self.assertEqual(result.returncode, 75)

    def test_valid_inherited_lock_descriptor_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            lock_path = root / "target" / "test-rust.lock"
            with worktree_lock.exclusive_worktree_lock(lock_path):
                self.assertTrue(worktree_lock.inherited_lock_is_valid(lock_path))
                with patch.object(run_conformance, "repo_root", return_value=root):
                    with patch.object(
                        run_conformance,
                        "ensure_harness_deps",
                        side_effect=lambda _: self.assertTrue(
                            worktree_lock.inherited_lock_is_valid(lock_path)
                        ),
                    ) as ensure_mock:
                        with patch.object(
                            run_conformance, "_run_owned", return_value=0
                        ) as run_mock:
                            self.assertEqual(run_conformance.main([]), 0)
            ensure_mock.assert_called_once_with(root)
            run_mock.assert_called_once()

    @staticmethod
    def _runner_command(root: Path) -> list[str]:
        source = (
            "import sys; from pathlib import Path; "
            f"sys.path.insert(0, {str(ROOT / 'conformance')!r}); "
            "import run_conformance; "
            f"run_conformance.repo_root = lambda: Path({str(root)!r}); "
            "raise SystemExit(run_conformance.main([]))"
        )
        return [sys.executable, "-c", source]


class WorktreeLockTests(unittest.TestCase):
    def test_whole_workflow_wrapper_fails_closed_on_windows(self) -> None:
        with patch.object(worktree_lock.os, "name", "nt"):
            with patch.object(worktree_lock.os, "execvpe") as exec_mock:
                self.assertEqual(
                    worktree_lock.main(["lock", sys.executable, "-c", "pass"]), 2
                )
        exec_mock.assert_not_called()

    def test_unlocked_same_inode_descriptor_cannot_borrow_another_owner(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            lock_path = Path(temp) / "gate.lock"
            holder = subprocess.Popen(
                [
                    sys.executable,
                    str(ROOT / "conformance" / "worktree_lock.py"),
                    str(lock_path),
                    sys.executable,
                    "-c",
                    "import time; time.sleep(2)",
                ]
            )
            try:
                time.sleep(0.1)
                with lock_path.open("a+b") as unlocked:
                    with patch.dict(
                        "os.environ",
                        {
                            worktree_lock.LOCK_FD_ENV: str(unlocked.fileno()),
                            worktree_lock.LOCK_OWNER_PID_ENV: str(os.getpid()),
                        },
                        clear=True,
                    ):
                        self.assertFalse(
                            worktree_lock.inherited_lock_is_valid(lock_path)
                        )
            finally:
                holder.terminate()
                holder.wait()

    @unittest.skipIf(os.name == "nt", "POSIX record-lock inheritance regression")
    def test_forked_descendant_cannot_orphan_the_worktree_lock(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            lock_path = Path(temp) / "gate.lock"
            helper = str(ROOT / "conformance" / "worktree_lock.py")
            fork_and_exit = (
                "import os,time; child=os.fork(); "
                "time.sleep(2) if child == 0 else None; os._exit(0)"
            )
            first = subprocess.run(
                [
                    sys.executable,
                    helper,
                    str(lock_path),
                    sys.executable,
                    "-c",
                    fork_and_exit,
                ],
                check=False,
            )
            second = subprocess.run(
                [
                    sys.executable,
                    helper,
                    str(lock_path),
                    sys.executable,
                    "-c",
                    "pass",
                ],
                check=False,
            )

            self.assertEqual(first.returncode, 0)
            self.assertEqual(second.returncode, 0)

    def test_locked_command_inherits_the_verifiable_descriptor(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            lock_path = Path(temp) / "gate.lock"
            command = [
                sys.executable,
                str(ROOT / "conformance" / "worktree_lock.py"),
                str(lock_path),
                sys.executable,
                str(ROOT / "conformance" / "worktree_lock.py"),
                "--check",
                str(lock_path),
            ]

            result = subprocess.run(command, check=False)

            self.assertEqual(result.returncode, 0)

    def test_competing_process_fails_before_running_its_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            lock_path = Path(temp) / "gate.lock"
            marker = Path(temp) / "command-ran"
            command = [
                sys.executable,
                str(ROOT / "conformance" / "worktree_lock.py"),
                str(lock_path),
                sys.executable,
                "-c",
                f"from pathlib import Path; Path({str(marker)!r}).touch()",
            ]

            with worktree_lock.exclusive_worktree_lock(lock_path):
                result = subprocess.run(command, check=False)

            self.assertEqual(result.returncode, 75)
            self.assertFalse(marker.exists())


class GeneratedReferenceTests(unittest.TestCase):
    def test_checked_in_conformance_references_match_the_live_report(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "gen_conformance_reference.py"),
                "--check",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(
            result.returncode,
            0,
            result.stdout + result.stderr,
        )


if __name__ == "__main__":
    unittest.main()


class GateSuiteFreshnessTests(unittest.TestCase):
    """The gate must refuse a score measured against a stale suite.

    Implements [CHKARCH-CONFORMANCE]. The whole point of cloning
    ``python/typing@main`` fresh is that a test upstream adds TODAY can fail us
    today. Grading an older tree still reports 100% while proving nothing, and
    three flags can reach one: ``--ref``, ``--suite-dir``, ``--reuse-clone``.
    """

    def test_current_upstream_tip_is_accepted(self) -> None:
        """The live ``main`` sha passes without touching the network twice."""
        live = "a" * 40
        completed = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=f"{live}\trefs/heads/main\n"
        )
        with patch("run_conformance.subprocess.run", return_value=completed):
            run_conformance.assert_graded_commit_is_live_main(
                {"sha": live, "short": live[:7], "date": "2026-08-04"}
            )

    def test_stale_graded_commit_fails_the_gate(self) -> None:
        """A suite behind the tip is a hard failure, not a pass."""
        completed = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=f"{'a' * 40}\trefs/heads/main\n"
        )
        with patch("run_conformance.subprocess.run", return_value=completed):
            with self.assertRaises(RuntimeError) as caught:
                run_conformance.assert_graded_commit_is_live_main(
                    {"sha": "b" * 40, "short": "bbbbbbb", "date": "2020-01-01"}
                )
        self.assertIn("STALE CONFORMANCE SUITE", str(caught.exception))

    def test_unreachable_upstream_fails_closed(self) -> None:
        """An unverifiable score is not a passing score — never skip the check."""
        with patch(
            "run_conformance.subprocess.run",
            side_effect=OSError("network down"),
        ):
            with self.assertRaises(RuntimeError) as caught:
                run_conformance.assert_graded_commit_is_live_main(
                    {"sha": "c" * 40, "short": "ccccccc", "date": "2026-08-04"}
                )
        self.assertIn("cannot verify", str(caught.exception))

    def test_empty_ls_remote_output_fails_closed(self) -> None:
        """A missing ref must not read as "nothing to compare, so pass"."""
        completed = subprocess.CompletedProcess(args=[], returncode=0, stdout="")
        with patch("run_conformance.subprocess.run", return_value=completed):
            with self.assertRaises(RuntimeError) as caught:
                run_conformance.assert_graded_commit_is_live_main(
                    {"sha": "d" * 40, "short": "ddddddd", "date": "2026-08-04"}
                )
        self.assertIn("could not resolve", str(caught.exception))

    def test_gate_mode_calls_the_freshness_check(self) -> None:
        """Wiring test: --gate must not be able to skip verification."""
        source = (ROOT / "conformance" / "run_conformance.py").read_text(
            encoding="utf-8"
        )
        gate_block = source.split('if not opts["gate"]:', 1)[1]
        self.assertIn("assert_graded_commit_is_live_main(commit)", gate_block)
