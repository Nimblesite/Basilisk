def sum_while(limit: int) -> int:
    total = 0
    i = 0
    while i < limit:
        def step(x: int) -> int:
            return x + 1
        i = step(i)
        total += i
    return total


def find_in_list(items: list[str], target: str) -> int:
    for idx, item in enumerate(items):
        def matches(a: str, b: str) -> bool:
            return a == b
        if matches(item, target):
            return idx
    return -1
