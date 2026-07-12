# Signal Box example project

Signal Box will be the book-owned, executable through-line. It will remain
dependency-light and target Python 3.12.

The first executable checkpoint now lives in [`signal-box/`](signal-box/).
Chapter 9 uses its explicit annotation policy and deliberately incomplete
functions to capture the real configuration editor and its path preview. Run
the capture reproducibly with `make -C book screenshots` from the repository
root.

Planned checkpoints:

1. one default argument-compatibility diagnostic;
2. everyday annotations, unions, and aliases;
3. narrowing and exhaustive routing;
4. `TypedDict` input transformed into a dataclass;
5. a storage protocol and generic report page;
6. a simulated untyped vendor package plus reviewed local stub;
7. explicit project rule policy;
8. bounded fixes and file adoption;
9. cross-file navigation and refactoring; and
10. tests, a debug scenario, a CPU hot path, and CI.

Every published checkpoint must agree with the governing Basilisk spec, pinned
release implementation, and executable tests. If those sources disagree, the
affected lesson is omitted until the repository resolves the discrepancy.
