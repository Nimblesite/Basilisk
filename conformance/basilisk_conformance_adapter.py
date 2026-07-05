# Basilisk adapter for the REAL python/typing conformance harness.
#
# This is the SAME per-checker adapter shape upstream ships for pyright, mypy,
# ty, pyrefly, zuban and pycroscope. The wheel-conformance CI/release gate
# ([CHKARCH-CONFORMANCE]) clones `python/typing@main`, appends this file to the
# suite's `conformance/src/type_checker.py`, then runs the suite's unmodified
# `src/main.py --only-run basilisk` against the pip-installed `basilisk-python`
# wheel. Every name used below (os, shutil, json, Path, Sequence, run, PIPE,
# CalledProcessError, TypeChecker) is already imported at the top of upstream's
# type_checker.py, so this file appends cleanly with no extra imports.
#
# It is byte-for-byte the adapter submitted upstream in the conformance PR — the
# gate proves the shipped wheel scores exactly what the submission claims.


class BasiliskTypeChecker(TypeChecker):
    @property
    def name(self) -> str:
        return "basilisk"

    def _executable(self) -> str:
        # Prefer the pip-installed `basilisk` console script (the wheel puts it on
        # PATH — this is how the suite's `uv sync` environment resolves it). A
        # BASILISK_BIN override is honoured for source builds / local dev.
        override = os.environ.get("BASILISK_BIN")
        if override:
            return override
        return shutil.which("basilisk") or "basilisk"

    def install(self) -> bool:
        try:
            self.get_version()
            return True
        except (CalledProcessError, FileNotFoundError):
            print(
                "Unable to run basilisk. Install it with "
                "`pip install basilisk-python` (or point BASILISK_BIN at the "
                "binary). See https://www.basilisk-python.dev."
            )
            return False

    def get_version(self) -> str:
        proc = run(
            [self._executable(), "--version"],
            check=True,
            stdout=PIPE,
            text=True,
        )
        return proc.stdout.strip()

    def run_tests(self, test_files: "Sequence[str]") -> dict[str, str]:
        # Basilisk emits a machine-readable JSON array of diagnostics. Each test
        # file is a self-contained module, so we check them individually and key
        # the flattened text output by file name for the scoring pass.
        results_dict: dict[str, str] = {}
        for test_file in test_files:
            proc = run(
                [
                    self._executable(),
                    "check",
                    test_file,
                    "--output",
                    "json",
                    "--color",
                    "never",
                ],
                stdout=PIPE,
                text=True,
                encoding="utf-8",
            )
            try:
                diagnostics = json.loads(proc.stdout) if proc.stdout.strip() else []
            except json.JSONDecodeError:
                diagnostics = []
            for diagnostic in diagnostics:
                file_name = Path(diagnostic.get("path", test_file)).name
                line_number = diagnostic.get("line", 0)
                col_number = diagnostic.get("col", 0)
                severity = diagnostic.get("severity", "error")
                message = diagnostic.get("message", "")
                code = diagnostic.get("code", "")
                line_text = (
                    f"{file_name}:{line_number}:{col_number}: "
                    f"{severity}: {message} [{code}]\n"
                )
                results_dict[file_name] = results_dict.get(file_name, "") + line_text
        return results_dict

    def parse_errors(self, output: "Sequence[str]") -> dict[int, list[str]]:
        # aliases_implicit.py:115:5: error: Invalid type expression ... [annotations_forward_refs]
        line_to_errors: dict[int, list[str]] = {}
        for line in output:
            if line.count(":") < 3:
                continue
            _, lineno, _col, rest = line.split(":", maxsplit=3)
            kind = rest.strip().split(":", 1)[0].strip()
            if kind not in ("error", "warning"):
                continue
            line_to_errors.setdefault(int(lineno), []).append(line)
        return line_to_errors


# Register Basilisk without depending on upstream's tuple contents: extend
# whatever `TYPE_CHECKERS` the suite defined. `--only-run basilisk` selects it.
TYPE_CHECKERS = tuple(TYPE_CHECKERS) + (BasiliskTypeChecker(),)
