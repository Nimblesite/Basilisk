def risky(value):
    try:
        return int(value)
    except ValueError:
        return 0


def also_risky(a, b):
    try:
        return a / b
    except ZeroDivisionError:
        return 0.0
