#!/usr/bin/env bash
# Runs the same steps as the verifier, on the visible tests.
set -euo pipefail
cd "$(dirname "$0")"

echo "== tests"
cargo test --release

echo "== compile checks"
./compile_checks/run.sh . compile_checks

echo "== miri"
# Tree Borrows: rayon's dependency crossbeam-epoch breaks the stricter,
# experimental Stacked Borrows rules. Data races are checked either way.
# Leaks are ignored because rayon's pool threads outlive the test.
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks" cargo +"${MIRI_TOOLCHAIN:-nightly}" miri test --test miri
