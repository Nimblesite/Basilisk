# Basilisk

Strict-by-default static type analyzer for Python. TypeScript for Python.

Every parameter must be typed. Every return type declared. `Any` is always explicit. No permissive modes.

Implemented in Rust — ships as a single binary, no Python runtime required.

---

## Try it

The `examples/` folder has ready-to-go Python files:

```sh
cargo run -- check examples/bad.py    # everything flagged
cargo run -- check examples/good.py   # clean
cargo run -- check examples/mixed.py  # some errors, some clean
cargo run -- check examples/          # all three at once
```

---

## Run it

### Without installing (development)

```sh
cargo run -- check path/to/file.py
cargo run -- check src/
cargo run -- check          # current directory
```

### Build and install

```sh
cargo build --release
# then put target/release/basilisk on your $PATH
basilisk check path/to/file.py
```

---

## Output

Diagnostics are printed in rustc style:

```
error[BSK-E0001]: Missing parameter type annotation for `data`
  --> src/utils.py:14:5
   |
14 | def process(data):
   |             ^^^^
   |
   = help: Add a type annotation: `data: <type>`
   = note: In Basilisk, all function parameters require explicit types
   = see: https://basilisk-lang.org/errors/BSK-E0001
```

### Exit codes

| Code | Meaning |
|------|---------|
| `0`  | Clean — no errors |
| `1`  | Type errors found |
| `3`  | Internal error |

---

## What gets flagged

All rules are on by default. There is no way to relax them globally.

### Annotation rules (E0001–E0005)

| Code | What triggers it |
|------|-----------------|
| `BSK-E0001` | Function parameter has no type annotation |
| `BSK-E0002` | Function is missing a return type annotation |
| `BSK-E0003` | Variable assignment has no type annotation |
| `BSK-E0004` | `*args` or `**kwargs` has no type annotation |
| `BSK-E0005` | Class attribute has no type annotation |

### Type correctness (E0010–E0025)

| Code | What triggers it |
|------|-----------------|
| `BSK-E0010` | Import from a module with no type stubs |
| `BSK-E0011` | Implicit `Any` — type cannot be inferred |
| `BSK-E0012` | Argument type does not match parameter type |
| `BSK-E0013` | Return type does not match declared return type |
| `BSK-E0014` | Assignment type does not match declared variable type |
| `BSK-E0015` | Wrong number of type arguments (e.g. `list[int, str]`) |
| `BSK-E0016` | Method override has incompatible signature |
| `BSK-E0017` | Class variable override has incompatible type |
| `BSK-E0018` | Reference to an undefined name |
| `BSK-E0019` | Variable used before it is assigned |
| `BSK-E0020` | `@overload` group has no non-decorated implementation |
| `BSK-E0021` | Two `@overload` signatures overlap |
| `BSK-E0022` | Dict key type is not hashable |
| `BSK-E0023` | `match` statement is not exhaustive |
| `BSK-E0024` | Type expression is not valid (e.g. `int | 42`) |
| `BSK-E0025` | Override method is missing the `@override` decorator |

---

## Quick example

**Before** — Basilisk rejects this:

```python
def greet(name):
    return "Hello " + name
```

```
error[BSK-E0001]: Missing parameter type annotation for `name`
  --> greet.py:1:10
   |
 1 | def greet(name):
   |           ^^^^
   |
   = help: Add a type annotation: `name: <type>`
   = see: https://basilisk-lang.org/errors/BSK-E0001

error[BSK-E0002]: Missing return type annotation
  --> greet.py:1:1
   |
 1 | def greet(name):
   | ^^^^^^^^^^^^^^^^
   |
   = see: https://basilisk-lang.org/errors/BSK-E0002
```

**After** — clean:

```python
def greet(name: str) -> str:
    return "Hello " + name
```

```
All checked. No issues found.
```

---

## Development

```sh
cargo build          # build
cargo test           # run all tests
cargo clippy         # lint
cargo fmt            # format
```

Rust 1.87+ required.
