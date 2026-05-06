#!/usr/bin/env python3
"""Enforce the cargo-mutants score ratchet stored in JSON.

Usage:
  python3 scripts/mutation_check_regression.py \
      <outcomes.json> <mutation_scores.json> [--scope working]
"""

from __future__ import annotations

import argparse
import pathlib
import sys


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from mutation_testing.mutants_report import (  # noqa: E402
    MutationScoreRegression,
    emit_regression_error,
    load_outcomes,
    record_score,
)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("outcomes", type=pathlib.Path)
    parser.add_argument("scores", type=pathlib.Path)
    parser.add_argument("--scope", default="working")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        record_score(load_outcomes(args.outcomes), args.scores, args.scope)
    except MutationScoreRegression as error:
        emit_regression_error(error)
        return 1
    except (OSError, ValueError, KeyError) as error:
        print(f"Mutation score check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
