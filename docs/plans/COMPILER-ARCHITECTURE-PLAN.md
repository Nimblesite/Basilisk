# Native compiler plan {#COMPILERPLAN}

## Status {#COMPILERPLAN-STATUS}

The repository has a checker-gated AST interpreter prototype. It has no typed HIR, LLVM or
other native backend, native runtime, compiler CLI integration, or native-binary tests. The
current behavior is documented in
[COMPILER-ARCHITECTURE-SPEC.md](../specs/COMPILER-ARCHITECTURE-SPEC.md); this plan owns the
unbuilt native compiler target.

## Native frontend boundary {#COMPILERPLAN-NATIVE-SCOPE}

- [ ] Define the typed-Python subset from implemented checker facts, with an explicit
  diagnostic for every unsupported construct.
- [ ] Decide which Python interactions cross an interpreter/CPython boundary rather than
  inventing native semantics.

## Typed HIR {#COMPILERPLAN-NATIVE-HIR}

- [ ] Replace the empty `hir.rs` placeholder with a typed IR derived from resolved/checker
  output.
- [ ] Specify lowering and verification for control flow, calls, classes, generics, unions,
  and errors before selecting concrete ABI layouts.

## Native code generation and JIT/AOT {#COMPILERPLAN-NATIVE-JIT}

- [ ] Select and prototype a maintained backend.
- [ ] Lower the verified HIR to runnable code with deterministic diagnostics for unsupported
  operations.
- [ ] Choose JIT, AOT, or both only after the backend prototype establishes the trade-offs.

## Runtime, layout, and memory {#COMPILERPLAN-NATIVE-RUNTIME}

- [ ] Define value, object, string, collection, exception, and class layouts from executable
  tests.
- [ ] Choose a memory strategy and concurrency contract from measured prototypes.

## Interop and CLI integration {#COMPILERPLAN-NATIVE-INTEROP}

- [ ] Define typed Python/native boundaries and packaging.
- [ ] Add compiler commands only after their library path produces a real artifact.

## Acceptance {#COMPILERPLAN-NATIVE-ACCEPTANCE}

- [ ] Tests compile and run native artifacts, not the AST interpreter.
- [ ] Unsupported Python fails explicitly; no construct silently changes meaning.
- [ ] Cross-platform CI covers every advertised backend and artifact format.
- [ ] Performance and compatibility claims are backed by committed reproducible benchmarks.
