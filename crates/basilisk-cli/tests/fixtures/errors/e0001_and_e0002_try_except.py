def risky(value: Any) -> None:
    try:
        return int(value)
    except ValueError:
        return 0


def also_risky(a: Any, b: Any) -> None:
    try:
        return a / b
    except ZeroDivisionError:
        return 0.0
