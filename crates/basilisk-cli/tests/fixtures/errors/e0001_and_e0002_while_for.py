def count(limit):
    total = 0
    while total < limit:
        total += 1
    return total


def search(items, target):
    for item in items:
        if item == target:
            return item
    return None
