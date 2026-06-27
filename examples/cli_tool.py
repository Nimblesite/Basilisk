"""
CLI tool — realistic command-line application with type violations.

This models the kind of ad-hoc scripting code that gradually grows
into a maintenance problem.  Every violation has a plausible story.

Run:  cargo run -- check examples/cli_tool.py
"""

from __future__ import annotations

import sys
from typing import Any, overload


# ── BSK-E0003: unannotated state at module scope ─────────────────────────────
_parsed_flags = {}  # BSK-E0003: empty dict
_positional_args = []  # BSK-E0003: empty list
_subcommand_map = {}  # BSK-E0003: empty dict


# ── BSK-E0001/E0002: argument parsing functions without any types ─────────────
def parse_flag(argv, name, default):  # BSK-E0001: three untyped params
    """Return the value of --name from argv, or default."""
    for i, arg in enumerate(argv):
        if arg == f"--{name}" and i + 1 < len(argv):
            return argv[i + 1]
    return default  # BSK-E0002: no return type


def run_subcommand(name, args, env):  # BSK-E0001: three untyped params
    handler = _subcommand_map.get(name)
    if handler:
        handler(args, env)
    # BSK-E0002: no return type


def format_error(code, message, context):  # BSK-E0001: three untyped params
    return f"[E{code}] {message} ({context})"  # BSK-E0002: no return type


# ── returns_compatibility: Any in public-facing output function ──────────────────────────
def print_result(
    value: Any,
) -> None:  # returns_compatibility: Any param, no justification
    print(value)


def load_config(path: str) -> Any:  # returns_compatibility: Any return
    return {}


# ── assignment_compatibility: exit code assigned a string, verbosity a float ────────────────
EXIT_SUCCESS: int = "0"  # assignment_compatibility: str assigned to int
EXIT_FAILURE: int = "1"  # assignment_compatibility: str assigned to int
DEFAULT_VERBOSITY: int = 1.5  # assignment_compatibility: float assigned to int


# ── classes_override_2: subcommand narrows timeout type incompatibly ──────────────────
class Command:
    name: str
    timeout: int
    retryable: bool


class NetworkCommand(Command):
    timeout: float = 30.0  # classes_override_2: float overrides int
    retryable: str = "yes"  # classes_override_2: str overrides bool


# ── names_undefined: reference before module-level assignment ──────────────────────
def get_version_string() -> str:
    return f"v{VERSION}"  # names_undefined: VERSION not yet defined


VERSION: str = "1.0.0"


# ── names_unbound: output path only bound inside a branch ────────────────────────
def resolve_output(flags: dict[str, str], default: bool) -> str:
    if "output" in flags:
        out_path = flags["output"]
    elif default:
        out_path = "/tmp/out.txt"
    # no else — out_path unbound if neither condition holds
    return out_path  # names_unbound: out_path may be unbound


# ── overloads_consistency: unannotated params make overloads identical ───────────────────
@overload
def coerce_value(raw, kind) -> int: ...  # BSK-E0001: raw, kind untyped


@overload
def coerce_value(raw, kind) -> int: ...  # BSK-E0001 + overloads_consistency: duplicate


def coerce_value(raw: str, kind: str) -> int:
    return int(raw)


# ── dict_key_hashable: list literal as a dict key (command alias map) ─────────────────
def default_aliases() -> dict[list[str], str]:
    return {["help", "h", "?"]: "help"}  # dict_key_hashable: list literal as key


# ── match_exhaustiveness: non-exhaustive match on log level ─────────────────────────────
def emit_log(level: str, msg: str) -> None:
    match level:
        case "info":
            print(f"[INFO]  {msg}")
        case "warn":
            print(f"[WARN]  {msg}", file=sys.stderr)
        case "error":
            print(f"[ERROR] {msg}", file=sys.stderr)
    # match_exhaustiveness: no wildcard — "debug", "trace" etc. are silently dropped


# ── BSK-E0025: override missing @override decorator ─────────────────────────
class BaseFormatter:
    def format(self, record: dict[str, str]) -> str:
        return str(record)


class JsonFormatter(BaseFormatter):
    indent: int = 2

    def format(self, record: dict[str, str]) -> str:  # BSK-E0025: no @override
        import json

        return json.dumps(record, indent=self.indent)
