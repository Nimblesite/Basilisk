def count(limit: Any) -> None:
    total = 0
    while total < limit:
        total += 1
    return total


def search(items: Any, target: Any) -> None:
    for item in items:
        if item == target:
            return item
    return None
