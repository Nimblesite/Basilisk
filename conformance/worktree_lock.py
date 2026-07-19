#!/usr/bin/env python3
"""Exclusive locks for conformance runners and POSIX workflow wrappers."""

from __future__ import annotations

import os
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

LOCK_FD_ENV = "BASILISK_TEST_RUST_LOCK_FD"
LOCK_OWNER_PID_ENV = "BASILISK_TEST_RUST_LOCK_OWNER_PID"


class WorktreeBusyError(RuntimeError):
    """Raised when another conformance workflow owns the worktree."""


def _acquire_descriptor(descriptor: int) -> None:
    if os.name == "nt":
        import msvcrt

        if os.lseek(descriptor, 0, os.SEEK_END) == 0:
            os.write(descriptor, b"\0")
        os.lseek(descriptor, 0, os.SEEK_SET)
        try:
            msvcrt.locking(descriptor, msvcrt.LK_NBLCK, 1)
        except OSError as exc:
            raise WorktreeBusyError("another conformance workflow is active") from exc
        return

    import fcntl

    try:
        # POSIX record locks survive exec in this process but are not inherited
        # by forked children, so an escaped descendant cannot orphan the lock.
        fcntl.lockf(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        raise WorktreeBusyError("another conformance workflow is active") from exc


def inherited_lock_is_valid(path: Path) -> bool:
    """Return whether the inherited descriptor is this path's active lock."""
    raw_descriptor = os.environ.get(LOCK_FD_ENV)
    raw_owner = os.environ.get(LOCK_OWNER_PID_ENV)
    try:
        descriptor = int(raw_descriptor or "")
        owner = int(raw_owner or "")
        inherited = os.fstat(descriptor)
        expected = path.stat()
    except (OSError, TypeError, ValueError):
        return False
    if (inherited.st_dev, inherited.st_ino) != (expected.st_dev, expected.st_ino):
        return False
    try:
        _acquire_descriptor(descriptor)
    except WorktreeBusyError:
        return owner == os.getppid()
    return owner == os.getpid() or os.name == "nt"


@contextmanager
def exclusive_worktree_lock(path: Path) -> Iterator[None]:
    """Hold a non-blocking OS lock until the protected work completes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as handle:
        _acquire_descriptor(handle.fileno())
        descriptor = handle.fileno()
        previous_descriptor = os.environ.get(LOCK_FD_ENV)
        previous_owner = os.environ.get(LOCK_OWNER_PID_ENV)
        os.set_inheritable(descriptor, True)
        os.environ[LOCK_FD_ENV] = str(descriptor)
        os.environ[LOCK_OWNER_PID_ENV] = str(os.getpid())
        try:
            yield
        finally:
            if previous_descriptor is None:
                os.environ.pop(LOCK_FD_ENV, None)
            else:
                os.environ[LOCK_FD_ENV] = previous_descriptor
            if previous_owner is None:
                os.environ.pop(LOCK_OWNER_PID_ENV, None)
            else:
                os.environ[LOCK_OWNER_PID_ENV] = previous_owner


def main(argv: list[str]) -> int:
    """Hold ``argv[0]`` while executing a POSIX workflow command."""
    if os.name == "nt":
        print(
            "FATAL: whole-workflow conformance locking requires POSIX record locks",
            file=sys.stderr,
        )
        return 2
    if len(argv) == 2 and argv[0] == "--check":
        return 0 if inherited_lock_is_valid(Path(argv[1])) else 1
    if len(argv) < 2:
        print("usage: worktree_lock.py LOCK COMMAND [ARGS...]", file=sys.stderr)
        return 2
    lock_path = Path(argv[0])
    command = argv[1:]
    try:
        with exclusive_worktree_lock(lock_path):
            os.execvpe(command[0], command, os.environ.copy())
    except WorktreeBusyError as exc:
        print(f"FATAL: {exc}", file=sys.stderr)
        return 75
    return 70


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
