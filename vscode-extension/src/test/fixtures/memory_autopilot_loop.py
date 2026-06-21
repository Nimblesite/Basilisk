"""Autopilot e2e fixture: leaks a fixed chunk on every loop iteration.

Mirrors examples/memory_demo.py's leak_cache: each pass over `leak_round`
retains ~1.5 MiB in the module-global CACHE, so the autopilot's per-pause
snapshot+diff sees steady growth at the SAME site and escalates leak confidence
LOW -> MEDIUM -> HIGH across passes. `Widget` exists so the reference-graph type
picker has a real class symbol to offer.
"""

CACHE = []


class Widget:
    """A retained object type for the reference-graph picker."""

    def __init__(self, index: int) -> None:
        self.index = index
        self.blob = bytes(1024)


def leak_round(index: int) -> int:
    for _i in range(300):
        CACHE.append("x" * 5000)  # ALLOC: ~1.5 MiB retained per round (the leak)
    CACHE.append(Widget(index))
    return len(CACHE)


def main() -> None:
    total = 0
    for index in range(8):
        total = leak_round(index)  # BREAKPOINT: autopilot captures on each pause
        print(f"round {index}: {total} cached")
    print("DONE", total)


if __name__ == "__main__":
    main()
