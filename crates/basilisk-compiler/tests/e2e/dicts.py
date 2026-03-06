def word_count(text: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for word in text.split():
        if word in counts:
            counts[word] = counts[word] + 1
        else:
            counts[word] = 1
    return counts


def invert(d: dict[str, int]) -> dict[int, list[str]]:
    result: dict[int, list[str]] = {}
    for key in d:
        val: int = d[key]
        if val not in result:
            result[val] = []
        result[val].append(key)
    return result


counts: dict[str, int] = word_count("the cat sat on the mat the cat")
print(sorted(counts.items()))

scores: dict[str, int] = {"alice": 95, "bob": 87, "carol": 95}
print(sorted(invert(scores).items()))
