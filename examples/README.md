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

## Spec rules vs house rules

Every **PEP** diagnostic below is a genuine violation of the
[Python typing spec](https://typing.python.org/en/latest/spec/index.html).
Those rules are on out of the box — in your own project, with no
configuration, you get exactly them, at `error`. A config file can grade one
of them down to `warning` or `info`, but no table may switch it off.

The rest are Basilisk's opt-in house rules (annotations required everywhere,
`@override` required, and so on). They stay silent until a `[tool.basilisk]`
table selects them. Basilisk resolves configuration per checked file by
walking up from the file's own folder, and the nearest table that decides a
rule wins outright. Two tables decide things here: the root `pyproject.toml`
selects the house rules for the repository, and `examples/pyproject.toml` — the
nearer one for everything under `examples/` — grades `BSK-0001`–`BSK-0005` and
`BSK-0025` down to `warning`. That is the incremental-adoption setup the docs
teach: warnings mean "this type-checks, but strictness isn't at full yet".
Rules the examples' table says nothing about, such as `BSK-0014` and
`BSK-0050`, are still decided by the root table.

That is the whole scoping mechanism. To run a rule at a different severity in
one part of a tree, put a `pyproject.toml` carrying its own `[tool.basilisk]`
table in that folder; there are no glob path patterns, no per-module tables,
and no presets or modes.

## Files

### Violation showcases (many diagnostics)

| File | Domain | PEP rules (always on, errors here) | Basilisk house rules (opt-in) |
|---|---|---|---|
| [bad.py](bad.py) | Minimal tour | `calls_argument_type`, `returns_compatibility`, `assignment_compatibility`, `calls_argument_count`, `classes_override`, `names_unbound`, `match_exhaustiveness` | BSK-0001, BSK-0002, BSK-0004 |
| [mixed.py](mixed.py) | Mixed typed / untyped | `calls_argument_type` | BSK-0001, BSK-0002 |
| [api_server.py](api_server.py) | REST API handler | `assignment_compatibility`, `overloads_consistency`, `names_unbound`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [data_pipeline.py](data_pipeline.py) | ETL pipeline | `assignment_compatibility`, `overloads_consistency`, `names_unbound`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014 |
| [ml_trainer.py](ml_trainer.py) | ML training loop | `assignment_compatibility`, `overloads_consistency`, `match_exhaustiveness`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [finance.py](finance.py) | Financial calculations | `assignment_compatibility`, `classes_override_2`, `overloads_consistency`, `names_unbound`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [cli_tool.py](cli_tool.py) | CLI application | `assignment_compatibility`, `classes_override_2`, `overloads_consistency`, `names_unbound`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [weird_violations.py](weird_violations.py) | Subtle edge cases | `overloads_consistency`, `names_unbound`, `classes_override_2`, `assignment_compatibility`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0014, BSK-0050 |

### Clean counterparts (zero diagnostics)

| File | Counterpart |
|---|---|
| [good.py](good.py) | `bad.py` fixed — passes at full strictness |
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

## Rule reference

Every diagnostic ends with a `see:` link to its documentation page. The full
catalog lives at [basilisk-python.dev/docs/rules](https://www.basilisk-python.dev/docs/rules/).

### PEP typing-spec rules shown here (always on, errors here)

| Code | Meaning |
|---|---|
| `calls_argument_type` | Argument incompatible with the parameter's declared type |
| `calls_argument_count` | Wrong number of arguments in a call |
| `returns_compatibility` / `returns_compatibility_2` | Returned value not assignable to the declared return type |
| `assignment_compatibility` | Assigned value not assignable to the annotation |
| `classes_override` | `@override` method incompatible with the base-class method |
| `classes_override_2` | Attribute override incompatible with the base class |
| `names_unbound` | Variable may be unbound on some execution paths |
| `match_exhaustiveness` | Non-exhaustive `match` — no wildcard `case _:` branch |
| `dict_key_hashable` | Unhashable type used as a dict key |
| `overloads_consistency` | Inconsistent or overlapping `@overload` group |

### Basilisk house rules shown here (opt-in)

The severity column is what the tables governing `examples/` select, not a
property of the code — a rule code carries no severity class. In an
unconfigured project none of these rules run at all.

| Code | Meaning | Severity here |
|---|---|---|
| BSK-0001 | Missing parameter type annotation | warning |
| BSK-0002 | Missing return type annotation | warning |
| BSK-0003 | Cannot infer type of empty collection or `None` | warning |
| BSK-0004 | Missing `*args` / `**kwargs` type annotation | warning |
| BSK-0025 | Override missing `@override` decorator | warning |
| BSK-0014 | Explicit `Any` without justification | warning |
| BSK-0050 | Redundant type annotation | warning |
