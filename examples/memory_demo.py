"""Basilisk Memory Profiling Demo — open this file and click
"Run & Track Memory (Current File)" in the Python Processes panel.

No breakpoint required: Basilisk starts tracemalloc at the entry pause, runs the
program to completion, and captures a final memory snapshot as it exits — so the
run ends in a viewable result (the V8 `.heapprofile` plus the purple allocation
heat map on the hot line), never a dead end.

The workload exercises every signal the memory profiler reports:

    leak_cache       -> retained forever in a module-global list (the leak)
    transient_spike  -> a big allocation that is freed before the program ends
    make_cycle       -> reference cycle with __del__, only the GC can reclaim

What to look for once the final snapshot opens:
  * `leak_cache`'s `bytes(512 * 1024)` line dominates — ~3 MiB still retained.
  * `transient_spike` shows up in PEAK memory but not in the final total.
  * The Node cycle survives until `gc.collect()` runs at the very end.

Prefer the interactive flow? Set a breakpoint on the marked line inside `main`'s
loop, launch under the debugger (F5), and take a snapshot each pass — the diff
between snapshots is where leak confidence escalates.
"""

import gc


# Module-level store that is never cleared — the classic accidental leak.
_LEAK: list[bytes] = []


class Node:
    """A graph node that points back at its owner, forming a reference cycle."""

    def __init__(self, label: str) -> None:
        self.label = label
        self.peer: Node | None = None  # Set to another Node to close the cycle.
        self.payload = bytearray(256 * 1024)  # Real bytes so the leak is visible.

    def __del__(self) -> None:
        # __del__ on a cycle historically blocked collection — a leak smell.
        print(f"collected node {self.label}")


def leak_cache(round_number: int) -> int:
    """Append to a module-global list that nothing ever frees."""
    chunk = bytes(512 * 1024)  # 512 KiB retained forever, one per round.
    _LEAK.append(chunk)
    return len(_LEAK)


def transient_spike() -> int:
    """Allocate a large buffer and drop it — peak rises, baseline does not."""
    scratch = [bytearray(1024 * 1024) for _ in range(8)]  # ~8 MiB, short-lived.
    total = sum(len(buffer) for buffer in scratch)
    return total  # `scratch` dies here; the next snapshot won't see it.


def make_cycle(label: str) -> None:
    """Build a two-node cycle that escapes reference counting."""
    left = Node(f"{label}-left")
    right = Node(f"{label}-right")
    left.peer = right
    right.peer = left  # Now neither node's refcount can ever reach zero.
    # Both go out of scope here, but the cycle keeps them alive until gc runs.


def main() -> None:
    for round_number in range(6):
        retained = leak_cache(round_number)  # Breakpoint here: snapshot each pass.
        spike = transient_spike()
        make_cycle(f"round{round_number}")
        print(
            f"round {round_number}: {retained} leaked chunks, {spike} transient bytes"
        )

    # Force a collection so the cycle's __del__ output appears at the end.
    unreachable = gc.collect()
    print(f"gc reclaimed {unreachable} objects")


if __name__ == "__main__":
    main()
