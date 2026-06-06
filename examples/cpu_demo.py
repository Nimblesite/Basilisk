"""Basilisk CPU Profiling Demo — open this file and start a profiling session.

Launch it under the Basilisk debugger (F5), then attach the CPU profiler to
the live session. The workload below is deliberately lopsided so the flame
chart, bottom-up table, and inline hot-line hints all have something to show:

    hot_primes()    -> dominates self-time  (trial division, no sieve)
    warm_strings()  -> moderate self-time   (quadratic string concat)
    cold_io()       -> almost all wall time in sleep, near-zero CPU

Things to look for once the .cpuprofile opens:
  * `is_prime` should be the heaviest leaf in the bottom-up view.
  * `fib_recursive` shows a deep, self-similar flame (exponential recursion).
  * `cold_io` barely appears — sampling profilers don't bill time spent asleep.
"""

import time


def is_prime(candidate: int) -> bool:
    """Deliberately naive primality test — the CPU hot spot of this demo."""
    if candidate < 2:
        return False
    divisor = 2
    while divisor * divisor <= candidate:  # Hot line: most samples land here.
        if candidate % divisor == 0:
            return False
        divisor += 1
    return True


def hot_primes(limit: int) -> list[int]:
    """Collect primes below `limit` the slow way to burn CPU on one function."""
    return [n for n in range(limit) if is_prime(n)]


def fib_recursive(n: int) -> int:
    """Exponential recursion — produces a tall, self-similar flame graph."""
    if n < 2:
        return n
    return fib_recursive(n - 1) + fib_recursive(n - 2)


def warm_strings(rows: int) -> str:
    """Quadratic string building — moderate, steady self-time."""
    report = ""
    for index in range(rows):
        report += f"row {index}: {'#' * (index % 40)}\n"  # Reallocates each pass.
    return report


def cold_io(rounds: int) -> int:
    """Mostly sleeping — shows how little CPU blocked I/O actually costs."""
    total = 0
    for _ in range(rounds):
        time.sleep(0.05)  # Wall time burns here, but the CPU profile stays flat.
        total += 1
    return total


def main() -> None:
    # Run a few rounds so the sampler accumulates a clear, stable picture.
    for round_number in range(5):
        primes = hot_primes(60_000)
        digest = fib_recursive(30)
        report = warm_strings(4_000)
        idle = cold_io(4)
        print(
            f"round {round_number}: "
            f"{len(primes)} primes, fib={digest}, "
            f"{len(report)} report chars, {idle} idle ticks"
        )


if __name__ == "__main__":
    main()
