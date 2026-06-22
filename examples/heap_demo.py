"""Basilisk Heap-Profile Demo — open this file and click
"Run & Track Memory (Current File)" in the Python Processes panel.

A deliberately *chunky* memory workload: it builds an in-memory analytics cache
of tens of megabytes across many distinct allocation sites, then keeps all of it
alive in the module-global WAREHOUSE until the program exits. The run needs no
breakpoint — Basilisk starts tracemalloc at the entry pause, runs to completion,
and captures a final snapshot as the program exits ([PROFILE-MEMORY-FINAL]).

Why this makes a good `.heapprofile`: Basilisk filters the debugger's own
allocations out and keeps each allocation's full call stack, so the viewer shows
a real call tree of *your* code — `warm_cache` branching into each builder below,
down to the line that allocated. With big buffers, medium columns, and a long
tail of small structures, the flame chart and the Self-Size table fill with real,
varied entries instead of a single dominant bar:

    allocate_frame_buffers -> a few large contiguous bytearrays (the wide bars)
    build_*_series         -> medium lists of numbers (the mid bars)
    build_inverted_index   -> thousands of small strings + posting lists (tail)
    build_session_records  -> objects with per-record blobs
    build_adjacency_graph  -> nested dict/list structure

What to look for once the snapshot opens:
  * The 8 MiB / 16 MiB buffers dominate the flame chart's widest slices.
  * Each `build_*` line shows up separately in the bottom-up (Self-Size) table.
  * Peak vs current: only WAREHOUSE survives, so the final total is what's live.
"""

from __future__ import annotations

# Everything is retained here, so the at-exit snapshot attributes each megabyte
# to the line that allocated it. Nothing is ever evicted.
WAREHOUSE: dict[str, object] = {}

# ── Large contiguous buffers (the wide flame-chart bars) ────────────────────


def allocate_frame_buffers() -> dict[str, bytearray]:
    """A render cache of differently-sized buffers — one big slice per line."""
    return {
        "rgba_canvas": bytearray(8 * 1024 * 1024),  # 8 MiB — the widest bar.
        "depth_buffer": bytearray(4 * 1024 * 1024),  # 4 MiB.
        "shadow_map": bytearray(2 * 1024 * 1024),  # 2 MiB.
        "lightmap": bytearray(1 * 1024 * 1024),  # 1 MiB.
    }


def allocate_embedding_matrix(vectors: int, dimensions: int) -> bytearray:
    """A flat float32 matrix as raw bytes — one large, clean allocation."""
    return bytearray(vectors * dimensions * 4)  # 4 bytes per float32 cell.


# ── Medium columnar series (the mid bars) ───────────────────────────────────


def build_price_series(rows: int) -> list[float]:
    """Distinct float objects (not interned) — a real per-line allocation."""
    return [float(index) * 1.5 + 0.25 for index in range(rows)]


def build_volume_series(rows: int) -> list[int]:
    """Big ints (above the small-int cache) — each one really allocated."""
    return [index * index + 9_999_999 for index in range(rows)]


def build_label_series(rows: int) -> list[str]:
    """Many short, distinct strings — a fat slice of small allocations."""
    return [f"row-{index:07d}" for index in range(rows)]


# ── Long tail of small structures (fills the bottom-up table) ───────────────


def synth_terms(doc_id: int, terms_per_doc: int) -> list[str]:
    """Synthesize a document's tokens — distinct interned-busting strings."""
    return [
        f"t{(doc_id * 131 + position) % 4096:04x}" for position in range(terms_per_doc)
    ]


def build_inverted_index(documents: int, terms_per_doc: int) -> dict[str, list[int]]:
    """token -> posting list. Thousands of tiny strings and lists."""
    index: dict[str, list[int]] = {}
    for doc_id in range(documents):
        for term in synth_terms(doc_id, terms_per_doc):
            index.setdefault(term, []).append(doc_id)
    return index


class SessionRecord:
    """A per-session object carrying its own payload — objects with __dict__."""

    def __init__(self, session_id: int) -> None:
        self.session_id = session_id
        self.token = f"sess-{session_id:08x}"
        self.payload = bytes(8 * 1024)  # 8 KiB blob retained per record.


def build_session_records(count: int) -> list[SessionRecord]:
    """A list of objects, each holding an 8 KiB blob — a chunky mid slice."""
    return [SessionRecord(session_id) for session_id in range(count)]


def build_adjacency_graph(nodes: int, fan_out: int) -> dict[int, list[int]]:
    """node -> neighbours. A nested dict/list structure with many small lists."""
    return {
        node: [(node * 2_654_435_761 + step) % nodes for step in range(fan_out)]
        for node in range(nodes)
    }


# ── Orchestration ───────────────────────────────────────────────────────────


def warm_cache() -> None:
    """Build every subsystem and retain it, so the heap fills up for real."""
    WAREHOUSE["frame_buffers"] = allocate_frame_buffers()
    WAREHOUSE["embeddings"] = allocate_embedding_matrix(vectors=65_536, dimensions=64)
    WAREHOUSE["price"] = build_price_series(120_000)
    WAREHOUSE["volume"] = build_volume_series(120_000)
    WAREHOUSE["labels"] = build_label_series(120_000)
    WAREHOUSE["index"] = build_inverted_index(documents=4_000, terms_per_doc=48)
    WAREHOUSE["sessions"] = build_session_records(2_000)
    WAREHOUSE["graph"] = build_adjacency_graph(nodes=8_000, fan_out=6)


def describe() -> str:
    """A one-line summary so the run prints something on completion."""
    parts = [f"{name}={type(value).__name__}" for name, value in WAREHOUSE.items()]
    return "warehouse: " + ", ".join(parts)


def main() -> None:
    warm_cache()
    print(describe())
    print(f"Loaded {len(WAREHOUSE)} retained subsystems — heap is warm for profiling.")


if __name__ == "__main__":
    main()
