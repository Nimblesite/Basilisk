"""Basilisk Exception Demo — set breakpoints inside except blocks to inspect exceptions.

Press F5 with this file open. Put breakpoints on the `print(...)` lines
inside each `except` block, then check the Variables pane for `exc`.
"""


class ValidationError(Exception):
    """Custom exception with extra attributes."""

    def __init__(self, field: str, message: str) -> None:
        super().__init__(f"{field}: {message}")
        self.field = field
        self.message = message


def main() -> None:
    # 1. KeyError — inspect exc, exc.args
    try:
        data: dict[str, int] = {"a": 1, "b": 2}
        value = data["missing_key"]
    except KeyError as exc:
        print(f"Caught KeyError: {exc}")  # breakpoint here

    # 2. ZeroDivisionError — inspect exc.args[0]
    try:
        result = 100 / 0
    except ZeroDivisionError as exc:
        print(f"Caught ZeroDivisionError: {exc}")  # breakpoint here

    # 3. IndexError — inspect type(exc), exc.args
    try:
        numbers: list[int] = [10, 20, 30]
        bad = numbers[99]
    except IndexError as exc:
        print(f"Caught IndexError: {exc}")  # breakpoint here

    # 4. ValueError with chained exception — inspect exc.__cause__
    try:
        try:
            int("not_a_number")
        except ValueError as original:
            raise ValueError("Failed to parse config") from original
    except ValueError as exc:
        print(f"Caught chained ValueError: {exc}")  # breakpoint here
        print(f"  Original cause: {exc.__cause__}")

    # 5. Custom exception — inspect exc.field, exc.message
    try:
        raise ValidationError("age", "must be >= 0")
    except ValidationError as exc:
        print(f"Caught ValidationError: {exc}")  # breakpoint here
        print(f"  field={exc.field}, message={exc.message}")

    print("\nAll exceptions handled successfully.")


if __name__ == "__main__":
    main()
