# Chapter 8 checkpoint

This checkpoint separates a simulated untyped runtime dependency from the type
contract Signal Box reviews and owns locally:

- `vendor/vendor_sensor.py` is the runtime package;
- `generated/vendor_sensor.pyi` records Basilisk 0.39.0's normalized generated
  starting point (the cache-specific hash line is omitted);
- `stubs/vendor_sensor.pyi` is the reviewed step-1 override; and
- `src/signal_box/vendor_readings.py` consumes the imported contract.

Run the runtime evidence from this directory:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src:vendor \
  python3 -m unittest discover -s tests -v
```

Generate a fresh best-effort stub with the documented release binary:

```sh
PYTHONPATH=vendor basilisk stubs generate vendor_sensor --python python3
```

The command writes `.basilisk/stubs/vendor_sensor.pyi`. The configured
`stub-paths = ["stubs"]` entry is searched first, so the reviewed stub remains
the contract used by the final check:

```sh
basilisk check --color never .
```
