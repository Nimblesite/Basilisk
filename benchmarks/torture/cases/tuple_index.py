"""Fixed-length tuple indexing, including through a contextual lambda — #284.

Indexing a fixed-length tuple with an out-of-range literal integer is a type
error (https://typing.python.org/en/latest/spec/tuples.html), so `two[2]` is
a required error. The lambda is the #284 false-positive shape: `pair` is
contextually a 3-tuple via `sorted`'s key parameter, so `pair[2]` is in
range — a checker that models the key parameter as a 2-tuple (or loses the
element count) reports a false positive and fails the case.
"""

items: list[tuple[str, int, float]] = [("a", 1, 1.0)]
in_order = sorted(items, key=lambda pair: (pair[1], pair[2], pair[0]))

two: tuple[int, str] = (1, "a")
bad = two[2]  # E
