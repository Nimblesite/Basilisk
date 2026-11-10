"""Memory-introspection e2e fixture: a retained object graph + a dropped cycle.

`build_registry` keeps a module-global list of custom-typed `Widget`s alive — a
real retention root the reference-graph walk can target. `make_cycle` builds two
linked `CycleNode`s and drops them, leaving an unreachable finalizer cycle on the
heap. `gc.disable()` keeps that cycle uncollected until the e2e drives a manual
`gc.collect()` through the gc-collect courier, so the collection is deterministic.
"""
import gc

# The e2e owns collection timing: with the automatic collector off, the dropped
# cycle in make_cycle() survives on the heap until the gc-collect courier runs.
gc.disable()


class Widget:
    """A retained, custom-typed object the reference-graph walk targets."""

    def __init__(self, label):
        self.label = label


class CycleNode:
    """A finalizer node; two linked instances form an unreachable cycle."""

    def __init__(self, name):
        self.name = name
        self.peer = None

    def __del__(self):
        pass


REGISTRY = []  # module global: a real retention root for the Widgets


def build_registry():
    for index in range(8):
        REGISTRY.append(Widget(f"w{index}"))


def make_cycle():
    left = CycleNode("left")
    right = CycleNode("right")
    left.peer = right
    right.peer = left
    # left/right drop on return -> an unreachable finalizer cycle on the heap.


def main():
    build_registry()
    ready = True       # BP_TRACK: start tracking + walk references here
    make_cycle()
    done = ready       # BP_GC: force a collection here
    return done


if __name__ == "__main__":
    main()
