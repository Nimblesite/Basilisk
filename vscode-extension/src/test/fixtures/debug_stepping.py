"""Test fixture for debug stepping E2E tests.

Each function has known variable values at specific lines.
The test harness sets breakpoints, steps through, and asserts
variable values match expectations.
"""


def arithmetic():
    """Simple arithmetic — step through and check values."""
    x = 10  # line 11
    y = 20  # line 12
    z = x + y  # line 13 — z should be 30
    w = z * 2  # line 14 — w should be 60
    result = w - 5  # line 15 — result should be 55
    return result  # line 16


def string_ops():
    """String operations — check string values after each step."""
    greeting = "hello"  # line 21
    name = "world"  # line 22
    message = greeting + " " + name  # line 23 — "hello world"
    upper = message.upper()  # line 24 — "HELLO WORLD"
    length = len(upper)  # line 25 — 11
    return upper  # line 26


def list_ops():
    """List mutations — verify list contents after each step."""
    items = [1, 2, 3]  # line 31
    items.append(4)  # line 32 — [1, 2, 3, 4]
    items.insert(0, 0)  # line 33 — [0, 1, 2, 3, 4]
    total = sum(items)  # line 34 — 10
    count = len(items)  # line 35 — 5
    return total  # line 36


def dict_ops():
    """Dictionary operations — check key/value pairs."""
    data = {"a": 1, "b": 2}  # line 41
    data["c"] = 3  # line 42
    keys = list(data.keys())  # line 43 — ["a", "b", "c"]
    total = sum(data.values())  # line 44 — 6
    has_a = "a" in data  # line 45 — True
    return total  # line 46


def nested_call():
    """Function calls — step into helper and back."""
    a = 5  # line 51
    b = double(a)  # line 52 — b should be 10
    c = double(b)  # line 53 — c should be 20
    return c  # line 54


def double(n):
    """Helper for nested_call."""
    result = n * 2  # line 59
    return result  # line 60


def loop_and_accumulate():
    """Loop stepping — verify accumulator at each iteration."""
    total = 0  # line 65
    for i in range(5):  # line 66
        total += i  # line 67
    # After loop: total = 0+1+2+3+4 = 10
    return total  # line 69


def conditional_branches():
    """Branching — verify which branch executes."""
    x = 42  # line 74
    if x > 100:  # line 75
        label = "big"  # line 76
    elif x > 10:  # line 77
        label = "medium"  # line 78
    else:
        label = "small"  # line 80
    return label  # line 81


def exception_handling():
    """Try/except — verify exception info."""
    caught = False  # line 86
    error_msg = ""  # line 87
    try:
        value = 1 / 0  # line 89
    except ZeroDivisionError as exc:
        caught = True  # line 91
        error_msg = str(exc)  # line 92
    return caught  # line 93


def type_variety():
    """Various types — verify type representations."""
    an_int = 42  # line 98
    a_float = 3.14  # line 99
    a_bool = True  # line 100
    a_none = None  # line 101
    a_tuple = (1, "two", 3.0)  # line 102
    a_set = {10, 20, 30}  # line 103
    a_bytes = b"hello"  # line 104
    return an_int  # line 105


class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

    def magnitude(self) -> float:
        return (self.x**2 + self.y**2) ** 0.5


def class_instance() -> float:
    """Class instantiation — check object attributes."""
    p = Point(3, 4)  # line 119
    mag = p.magnitude()  # line 120 — 5.0
    return mag  # line 121


# Entry point — run all functions so the script completes.
if __name__ == "__main__":
    arithmetic()
    string_ops()
    list_ops()
    dict_ops()
    nested_call()
    loop_and_accumulate()
    conditional_branches()
    exception_handling()
    type_variety()
    class_instance()
    print("All debug fixtures executed.")
