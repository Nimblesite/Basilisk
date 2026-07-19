#!/usr/bin/env python3
"""Select the exact PEP 639 License-Expression for one release wheel target."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    manifest = json.loads((ROOT / "runtime-license-manifest.json").read_text())
    try:
        expression = manifest["wheel_license_expressions"][args.target]
    except KeyError as error:
        parser.error(f"unsupported release target: {args.target}")
        raise AssertionError("argparse exits") from error

    pyproject = ROOT / "pyproject.toml"
    text = pyproject.read_text()
    updated, count = re.subn(
        r'(?m)^license = ".*"$',
        f'license = "{expression}"',
        text,
        count=1,
    )
    if count != 1:
        raise SystemExit("pyproject.toml must contain exactly one project license")
    pyproject.write_text(updated)


if __name__ == "__main__":
    main()
