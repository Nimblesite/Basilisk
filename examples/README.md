<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

# Basilisk Examples

Realistic Python scripts that demonstrate what Basilisk catches — and what
clean, fully-typed code looks like.

## Running the examples

```bash
# Check a single file
basilisk check examples/bad.py

# Check every example at once
basilisk check examples/

# JSON output (for editors / CI)
basilisk check examples/bad.py --output json
```

## Files

### Violation showcases (many diagnostics)

| File | Domain | Notable violations |
|---|---|---|
| [bad.py](bad.py) | Minimal toy | E0001, E0002, E0004, E0005 |
| [mixed.py](mixed.py) | Mixed typed / untyped | E0001, E0002 |
| [api_server.py](api_server.py) | REST API handler | E0001–E0003, E0011, E0014, E0017, E0019, E0025 |
| [data_pipeline.py](data_pipeline.py) | ETL pipeline | E0001–E0003, E0011, E0014, E0017, E0019, E0022, E0025 |
| [ml_trainer.py](ml_trainer.py) | ML training loop | E0001–E0003, E0011, E0014, E0017, E0018, E0023, E0025 |
| [finance.py](finance.py) | Financial calculations | E0001–E0003, E0011, E0014, E0017, E0018, E0019, E0023, E0025 |
| [cli_tool.py](cli_tool.py) | CLI application | E0001–E0003, E0011, E0014, E0017, E0019, E0023, E0025 |
| [weird_violations.py](weird_violations.py) | Subtle edge cases | E0003, E0011, E0014, E0017, E0019, E0021, E0023, E0025 |

### Clean counterparts (zero diagnostics)

| File | Counterpart |
|---|---|
| [good.py](good.py) | Minimal toy fixed |
| [api_server_clean.py](api_server_clean.py) | `api_server.py` fixed |

### Debugger & profiler demos (launch with F5)

These are clean, fully-typed scripts meant to be *run* under the Basilisk
debugger rather than statically checked. Open one and press F5.

| File | Demonstrates | How to use |
|---|---|---|
| [debug_demo.py](debug_demo.py) | Breakpoints, Watch panel, Locals, Debug Console | Set a breakpoint and step through |
| [profile_demo.py](profile_demo.py) | CPU profiling — a few seconds of CPU-bound work with a clear hot spot, so the flame chart and hot-line heat map fill in | One click: **Run & Profile CPU (Current File)** |
| [cpu_demo.py](cpu_demo.py) | CPU sampling — hot/warm/cold flame chart, hot-line hints | Attach the CPU profiler to the live session |
| [memory_demo.py](memory_demo.py) | Memory — sustained leak, transient spike, reference cycle; the run captures a final snapshot at exit, so it ends in a viewable heat map / `.heapprofile` | One click: **Run & Track Memory (Current File)** |
| [heap_demo.py](heap_demo.py) | Memory — a chunky ~70 MB warm cache across ~40 distinct allocation sites, so the `.heapprofile` flame chart and Self-Size table fill with varied, real slices | One click: **Run & Track Memory (Current File)** |

## Error code reference

| Code | Meaning |
|---|---|
| E0001 | Missing parameter type annotation |
| E0002 | Missing return type annotation |
| E0003 | Cannot infer type of empty collection or `None` |
| E0004 | Missing `*args` / `**kwargs` type annotation |
| E0005 | Missing class attribute annotation |
| E0010 | Untyped import |
| E0011 | Explicit `Any` without justification comment |
| E0012 | Wrong argument type passed to a function |
| E0013 | `-> None` function returns a non-None value |
| E0014 | Assignment type mismatch |
| E0015 | Invalid type argument |
| E0016 | Method signature incompatible override |
| E0017 | Attribute type incompatible override |
| E0018 | Variable used before it is defined |
| E0019 | Variable may be unbound on some code paths |
| E0020 | `@overload` group missing implementation |
| E0021 | Overlapping overload signatures |
| E0022 | Unhashable type used as dict key |
| E0023 | Non-exhaustive `match` statement |
| E0024 | Invalid type form |
| E0025 | Override missing `@override` decorator |
