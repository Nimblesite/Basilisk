# Signal Box example project

Signal Box will be the book-owned, executable through-line. It will remain
dependency-light and avoid a project-wide Python-version assumption. A
checkpoint may use a versioned feature only when the lesson names its governing
PEP or language boundary.

Chapter 4's executable checkpoint lives in
[`ch04-type-vocabulary/`](ch04-type-vocabulary/). Chapter 5 continues with
assignment, function, collection, and callback compatibility in
[`ch05-compatibility/`](ch05-compatibility/). Chapter 6 follows inference,
narrowing, a user-defined type guard, and exhaustive routing in
[`ch06-narrowing/`](ch06-narrowing/). Chapter 7 separates validated external
data, domain models, storage behavior, and generic report pages in
[`ch07-structured-contracts/`](ch07-structured-contracts/). Chapter 8 separates
an untyped runtime dependency from its generated and reviewed contracts in
[`ch08-imports-and-stubs/`](ch08-imports-and-stubs/). Chapter 9 uses
[`signal-box/`](signal-box/) with its explicit annotation policy and
deliberately incomplete functions to capture the real configuration editor and
its path preview. Run that capture reproducibly with `make -C book screenshots`
from the repository root.

Planned checkpoints:

1. one default argument-compatibility diagnostic;
2. everyday annotations, unions, and aliases;
3. assignments, function contracts, mutable collections, and callbacks;
4. narrowing and exhaustive routing;
5. a simulated untyped vendor package plus reviewed local stub;
6. explicit project rule policy;
7. bounded fixes and file adoption;
8. cross-file navigation and refactoring; and
9. tests, a debug scenario, a CPU hot path, and CI.

Every published checkpoint must agree with the governing Basilisk spec, pinned
release implementation, and executable tests. If those sources disagree, the
affected lesson is omitted until the repository resolves the discrepancy.
