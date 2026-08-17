def safe_open(path: str) -> str:
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""


def safe_divide(a: float, b: float) -> float:
    try:
        return a / b
    except ZeroDivisionError:
        return 0.0
    finally:
        pass


def safe_int(value: str) -> int:
    try:
        return int(value)
    except (ValueError, TypeError):
        return 0
