# Basilisk compiler prototype {#COMPARCH}

The current `basilisk-compiler` crate is an experimental, dynamically typed AST interpreter
behind the checker gate. It does not produce native code, expose compiler CLI commands, or
implement the native architecture tracked in
[COMPILER-ARCHITECTURE-PLAN.md](../plans/COMPILER-ARCHITECTURE-PLAN.md).

## Analysis and execution pipeline {#COMPILER-PIPELINE}

`compile_and_run` performs four steps:

1. parse source with `basilisk-parser`;
2. resolve names with `basilisk-resolver`;
3. run `basilisk-checker` and stop before execution if any error diagnostic exists;
4. interpret the Ruff AST and capture calls to `print` as a string.

Parse, resolution, and interpreter failures return `CompileError`. Checker failures return a
successful `CompileResult` containing only error diagnostics and empty stdout. Warnings do
not block execution. The resolved module currently supplies the gate but does not drive
runtime representation or dispatch.

## Interpreter backend {#COMPILER-CODEGEN}

`crates/basilisk-compiler/src/codegen.rs` stores values dynamically as `None`, booleans,
`i64`, `f64`, UTF-8 strings, lists, dictionaries, tuples, functions, lambdas, classes,
instances, and bound methods.

Fixture-backed statement support includes expression statements, returns, assignment,
annotated and augmented assignment, `if`, `for`, `while`, function and basic class
definitions, `pass`, `break`, and `continue`. Expressions cover the literals and containers
above, arithmetic/comparison/boolean operations, calls, attributes, subscripts, lambdas,
assignment expressions, conditional expressions, f-strings, and the builtins exercised by
the test fixtures.

This is deliberately not presented as general Python execution:

- imports are ignored rather than loaded;
- unsupported statements return an interpreter error;
- unsupported expression variants currently evaluate to `None`;
- calls model regular positional parameters only;
- classes retain methods and instance attributes but do not implement bases, decorators,
  MRO, protocols, or static layouts;
- `isinstance` is a placeholder that always returns true.

### Integer behavior {#COMPILER-TYPES-INT}

Integers are signed 64-bit values. Literals outside that range fail with `int too large`.
There is no arbitrary-precision or configurable big-integer mode.

### String behavior {#COMPILER-TYPES-STR}

Strings use Rust UTF-8 `String`. The fixture suite covers construction, formatting, methods,
indexing, and slicing, but the current `len` implementation counts bytes and indexing mixes
byte length with Unicode scalar selection. These are known interpreter limitations, not
language guarantees.

## End-to-end tests {#COMPILER-TESTING-E2E}

`crates/basilisk-compiler/tests/e2e_tests.rs` discovers `.py` fixtures, calls the library
entry point, and compares captured stdout exactly with `-expectedoutput.txt`. It tests the
interpreter pipeline; it does not build or launch a native executable.

### Committed examples {#COMPILER-TESTING-EXAMPLES}

The nine committed examples cover arithmetic, basic classes, closures, dictionaries, a
multi-feature program (`dostuff`), hello world, lists, recursion, and strings.

### Failure-fixture convention {#COMPILER-TESTING-FAILURES}

The harness recognizes `<name>-expectederror.txt` and checks that a returned diagnostic code
contains its contents. No failure fixture is currently committed, so this branch is a
supported test convention rather than verified product behavior.
