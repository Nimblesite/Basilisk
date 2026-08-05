"""Unannotated parameters used consistently — the #317 gradual posture.

Unannotated code is GRADUAL: parameters without annotations are implicitly
`Any`-typed and the typing spec mandates no diagnostic for their absence
(https://typing.python.org/en/latest/spec/type-system.html#the-gradual-guarantee
— strictness rules demanding annotations are opt-in house rules in every
checker's out-of-the-box configuration). The call and the arithmetic are
well-typed under any inference. Any diagnostic below is a false positive.
"""


def multiply(x, y) -> int:
    return x * y


result: int = multiply(4, 5)
