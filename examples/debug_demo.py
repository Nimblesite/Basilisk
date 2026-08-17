"""Basilisk Debug Demo — open this file and press F5 to try the debugger.

Set breakpoints on any line, then step through to see variables in the
Watch panel, Locals scope, and Debug Console.
"""


def fibonacci(n: int) -> list[int]:
    """Generate the first n Fibonacci numbers."""
    seq: list[int] = []
    a, b = 0, 1
    for _ in range(n):
        seq.append(a)  # Set a breakpoint here to watch the sequence grow
        a, b = b, a + b
    return seq


def classify_numbers(numbers: list[int]) -> dict[str, list[int]]:
    """Split a list into evens and odds."""
    result: dict[str, list[int]] = {"even": [], "odd": []}
    for num in numbers:
        if num % 2 == 0:
            result["even"].append(num)
        else:
            result["odd"].append(num)
    return result


def main() -> None:
    # Step through these lines and inspect variables in the Watch panel.
    name = "Basilisk"
    version = "0.1.0"
    greeting = f"Welcome to {name} v{version} debugger demo!"
    print(greeting)

    # Watch `fib` grow as you step through fibonacci().
    fib = fibonacci(10)
    print(f"Fibonacci(10): {fib}")

    # Inspect the classified dict in the Variables pane.
    classified = classify_numbers(fib)
    print(f"Even: {classified['even']}")
    print(f"Odd:  {classified['odd']}")

    # Try evaluating these in the Debug Console (REPL):
    #   sum(fib)
    #   len(classified["even"])
    #   [x ** 2 for x in fib]
    total = sum(fib)
    print(f"Sum: {total}")


if __name__ == "__main__":
    main()
