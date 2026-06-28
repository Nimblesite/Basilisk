# Basilisk Compiler — Implementation Plan {#COMPILERPLAN}

> **Spec**: [COMPILER-ARCHITECTURE-SPEC.md](../specs/COMPILER-ARCHITECTURE-SPEC.md) — read before touching any code.

---

## Status {#COMPILERPLAN-STATUS}

**Early / experimental — not started as a shippable compiler.** The spec describes a
native AOT compiler for the PEP-typed subset of Python. This plan turns that spec into an
ordered, checkable backlog. Nothing here is "missing functionality we forgot"; it is a
deliberately-unbuilt subsystem with a concrete design. Each TODO references the spec
section that defines it so the spec-ID web stays intact.

The compilation-gate invariant ([COMPILER-SUBSET]) governs everything: anything the type
checker can verify, the compiler must be able to compile or explicitly route to interop.

---

## TODO {#COMPILERPLAN-TODO}

### Phase C1 — Scope & boundary rules {#COMPILERPLAN-TODO-SCOPE}

- [ ] Lock the compilation surface: enumerate supported constructs ([COMPILER-SUPPORTED]) and the interop-routed exclusions with reasons ([COMPILER-EXCLUDED]).
- [ ] Specify boundary-case lowering rules: `isinstance` → O(1) tag compare, generators → state machines, C3 MRO at compile time ([COMPILER-BOUNDARY]).
- [ ] Enforce the compile-vs-verify gate ([COMPILER-SUBSET]) as a checker pass that flags un-compilable constructs in `compile` dirs.

### Phase C2 — Type representation & data layout {#COMPILERPLAN-TODO-LAYOUT}

- [ ] Implement the Python-type → compiled-representation/ABI mapping ([COMPILER-TYPES]).
- [ ] Class struct layout: type tag, refcount, optional vtable ptr, declaration-order fields with padding ([COMPILER-LAYOUT], [COMPILER-LAYOUT-CLASS]).
- [ ] Inheritance layout: single-inheritance embedding + multiple-inheritance C3-flattened with per-base vtables ([COMPILER-LAYOUT-INHERIT]).
- [ ] Protocol → vtable/fat-pointer lowering ([COMPILER-LAYOUT-PROTOCOLS]); `isinstance` via type-tag/ancestor-chain ([COMPILER-LAYOUT-ISINSTANCE]).

### Phase C3 — Memory management {#COMPILERPLAN-TODO-MEMORY}

- [ ] Define the swappable runtime memory interface (alloc/dealloc/incref/decref/collect), selected at link time ([COMPILER-MEMORY], [COMPILER-MEMORY-IFACE]).
- [ ] Default backend: CPython-style refcount + 3-generation cyclic GC ([COMPILER-MEMORY-DEFAULT]).
- [ ] Escape analysis for stack allocation of non-escaping primitives/small tuples/frozen dataclasses ([COMPILER-MEMORY-STACK]).
- [ ] Ownership-hint elision via `Borrowed`/`Owned`/`InOut` across backends ([COMPILER-MEMORY-OWNERSHIP]); document alternative backends (ARC/tracing/arena) as interface-only ([COMPILER-MEMORY-FUTURE]).

### Phase C4 — Runtime {#COMPILERPLAN-TODO-RUNTIME}

- [ ] Build `basilisk-runtime` crate: memory, strings, collections, exceptions, builtins ([COMPILER-RUNTIME], [COMPILER-RUNTIME-CORE]).
- [ ] No-GIL true parallelism via typed concurrency ([COMPILER-RUNTIME-NOGIL]).
- [ ] Exceptions via LLVM landing pads (invoke/landing-pad/finally, O(1) type dispatch, DWARF traces) ([COMPILER-RUNTIME-EXCEPTIONS]).

### Phase C5 — Interop {#COMPILERPLAN-TODO-INTEROP}

- [ ] Folder-convention compile-vs-interop split + `pyproject.toml` config + per-file overrides ([COMPILER-INTEROP], [COMPILER-INTEROP-LAYOUT]).
- [ ] Call Python from Basilisk via pyo3 with value-conversion table + boundary type-checking ([COMPILER-INTEROP-PY2BSK]).
- [ ] Export compiled modules as CPython extensions (PEP 384 stable ABI) ([COMPILER-INTEROP-BSK2PY]).
- [ ] Native compilation of PEP-compliant typed libraries ([COMPILER-INTEROP-LIBS]).

### Phase C6 — Stdlib & CLI {#COMPILERPLAN-TODO-STDLIB-CLI}

- [ ] Three-tier stdlib strategy with concrete per-tier module lists ([COMPILER-STDLIB]).
- [ ] `run`/`build` command surface, flag table, exit-code contract ([COMPILER-CLI], [COMPILER-CLI-FLAGS], [COMPILER-CLI-EXIT]).
- [ ] AOT pipeline + content-addressed caching ([COMPILER-MODES], [COMPILER-MODES-AOT], [COMPILER-MODES-CACHE]).

### Phase C7 — Crates, perf & testing {#COMPILERPLAN-TODO-INFRA}

- [ ] Create the new crates and keep the dependency graph acyclic ([COMPILER-CRATES], [COMPILER-CRATES-DEPS]).
- [ ] Track performance targets vs C/Rust ([COMPILER-PERF]).
- [ ] Stand up the E2E/integration/unit test layers ([COMPILER-TESTING], [COMPILER-TESTING-LAYERS]).
- [ ] Walk the phased delivery milestones C1–C10 ([COMPILER-ROADMAP]).
