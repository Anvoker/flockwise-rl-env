#!/usr/bin/env bash
# Full self-test of a task's verifier, each part in its own work directory
# (and so its own Cargo target directory: sharing one between different source
# trees can run stale binaries from another tree).
#
#   1. Alternative valid solutions must all pass.
#   2. Every mutant (known-bad solution) must fail at least one check.
#   3. The reference's false-failure rate over many seeds.
#
# Usage: tools/selftest.sh <task dir> <work dir> [calibration runs] [jobs]
#
# Deletes only inside <work dir>: the tools it calls recreate their
# subdirectories there.
set -uo pipefail
task=$1
work=$2
runs=${3:-1000}
jobs=${4:-6}
here=$(cd "$(dirname "$0")" && pwd)

mkdir -p "$work" && work=$(cd "$work" && pwd) || exit 1
case "$work" in
    "" | "/" | "/c" | "/c/") echo "refusing to use work dir '$work'" >&2; exit 1 ;;
esac
unset CARGO_TARGET_DIR

echo "== alternatives"
python -I "$here/mutants.py" "$task" "$work/alternatives" --alternatives
echo "== mutants"
python -I "$here/mutants.py" "$task" "$work/mutants"
echo "== calibration ($runs runs)"
bash "$here/calibrate.sh" "$task" "$runs" "$jobs" "$work/calibration"
