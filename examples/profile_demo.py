"""Basilisk CPU Profiling Demo — open this file and click
"Run & Profile CPU (Current File)" in the Python Processes panel.

Unlike debug_demo.py — which finishes in about a millisecond, far too fast for a
sampling profiler to catch — this program runs a CPU-bound workload for a few
seconds. That gives the sampler enough snapshots to build a real flame chart,
bottom-up table, and inline hot-line heat map.

The work is deliberately lopsided so the profile points straight at the
bottleneck:

    sum_of_squares()  -> the hot spot: a tight arithmetic loop
    count_primes()    -> moderate: naive trial-division primality
    format_round()    -> light: a little string formatting

What to look for once the profile opens:
  * sum_of_squares dominates the bottom-up (self-time) table.
  * The `total += index * index` line wears the brightest heat-map color.
  * count_primes is a clear but smaller slice; format_round barely registers.
"""

import time

# Run the workload for about this many seconds so the sampler builds a stable
# picture regardless of machine speed. A fixed iteration count would finish too
# fast on a quick machine (too few samples) and drag under the debugger's line
# tracing — a wall-clock deadline keeps the demo's profile meaningful either way.
PROFILE_SECONDS = 3.0


def sum_of_squares(count: int) -> int:
    """The hot spot: a tight arithmetic loop where most samples should land."""
    total = 0
    for index in range(count):
        total += index * index  # Hot line — the profiler paints this brightest.
    return total


def count_primes(limit: int) -> int:
    """Moderate cost: naive primality by trial division (no sieve)."""
    found = 0
    for candidate in range(2, limit):
        divisor = 2
        is_prime = True
        while divisor * divisor <= candidate:
            if candidate % divisor == 0:
                is_prime = False
                break
            divisor += 1
        if is_prime:
            found += 1
    return found


def format_round(round_number: int, squares: int, primes: int) -> str:
    """Light cost: a little string formatting per round."""
    return f"round {round_number}: sum_of_squares={squares:,}, primes={primes}"


def main() -> None:
    deadline = time.time() + PROFILE_SECONDS
    round_number = 0
    summary = ""
    while time.time() < deadline:
        squares = sum_of_squares(100_000)
        primes = count_primes(2_000)
        summary = format_round(round_number, squares, primes)
        round_number += 1
    print(summary)
    print(f"Profiled {round_number} rounds over ~{PROFILE_SECONDS:.0f}s of CPU work.")


if __name__ == "__main__":
    main()
