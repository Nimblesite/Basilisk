#!/usr/bin/env bash
# Independent verification of the conformance mutation result.
#
# Trusts nothing pre-existing: fresh clone of python/typing, sharkdp's gist
# downloaded straight from GitHub, your own release build of basilisk.
#
# Usage:  ./verify_mutation.sh
set -euo pipefail

WORK="${WORK:-$(mktemp -d)}"
BASILISK="$(pwd)/target/release/basilisk"

echo "workdir:  $WORK"
echo "binary:   $BASILISK"
[ -x "$BASILISK" ] || { echo "ERROR: build it first: cargo build --release --bin basilisk"; exit 1; }
echo "commit:   $(git rev-parse HEAD)"
echo "dirty:    $(git status --porcelain | wc -l | tr -d ' ') uncommitted files"
echo

cd "$WORK"

echo "==> cloning python/typing"
git clone -q --depth 1 https://github.com/python/typing.git pristine
echo "    suite @ $(git -C pristine rev-parse --short HEAD)"
cp -r pristine mutated

echo "==> downloading sharkdp's mutation script from the gist"
curl -sL "https://gist.githubusercontent.com/sharkdp/3f3266fd9c67d22137e2b6c015c5f206/raw/mutate_typing_conformance.py" -o mutate.py
echo "    sha256: $(shasum -a 256 mutate.py | cut -d' ' -f1)"
grep -q "def mutate_source" mutate.py || { echo "ERROR: gist download looks wrong"; exit 1; }

echo "==> applying mutations"
uv run --script mutate.py mutated/ | tail -1

for T in pristine mutated; do
  echo "==> running conformance: $T"
  # wipe committed upstream results so nothing can carry over
  rm -rf "$T/conformance/results/basilisk"
  ( cd "$T/conformance" && BASILISK_BIN="$BASILISK" uv run python src/main.py --only-run basilisk >/dev/null 2>&1 )
done

echo
echo "================ RESULT ================"
python3 - "$WORK" <<'PY'
import sys, tomllib, pathlib
work = pathlib.Path(sys.argv[1])
for label, d in (("PRISTINE", "pristine"), ("MUTATED ", "mutated")):
    p = work / d / "conformance/results/basilisk"
    # score the field the harness COMPUTES, not the human-curated `conformant`
    rows = []
    for f in sorted(p.glob("*.toml")):
        if f.stem == "version":
            continue
        v = tomllib.loads(f.read_text()).get("conformance_automated")
        if v:
            rows.append((f.stem, v))
    ok = sum(1 for _, v in rows if v == "Pass")
    print(f"{label}  {ok:3} pass / {len(rows)-ok:3} fail   (of {len(rows)})")
PY
echo "========================================"
echo
echo "Scored on 'conformance_automated' (computed by the harness)."
echo "NOT 'conformant' -- that field is human-curated and committed upstream;"
echo "reading it gives a false 100% because it carries over from the old results."
echo
echo "artifacts kept in: $WORK"
