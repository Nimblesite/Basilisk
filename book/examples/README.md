# Signal Box example project

Signal Box will be the book-owned, executable through-line. It will remain
dependency-light and avoid a project-wide Python-version assumption. A
checkpoint may use a versioned feature only when the lesson names its governing
PEP or language boundary.

Chapter 4's executable checkpoint lives in
[`ch04-type-vocabulary/`](ch04-type-vocabulary/). Chapter 9 uses
[`signal-box/`](signal-box/) with its explicit annotation policy and
deliberately incomplete functions to capture the real configuration editor and
its path preview. Run that capture reproducibly with `make -C book screenshots`
from the repository root.

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
