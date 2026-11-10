"""Self-referential class bases — the #398 hang reproducer.

A class name is not bound until the `class` statement completes, so using it
in its own bases list is an unbound-name error at evaluation time (Python
language semantics; `NameError` at runtime). The torture here is not the
diagnostic — it is TERMINATION: this fuzzed shape hung `basilisk check`
(https://github.com/Nimblesite/Basilisk/issues/398). A checker that spins
forever fails the case by timeout regardless of what it would have printed.
"""


class C(C[int], C[bool]):  # E
    pass


class D(D):  # E
    pass
