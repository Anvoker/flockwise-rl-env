#!/usr/bin/env bash
# Runs the same steps as the verifier, on the visible tests.
set -euo pipefail
cd "$(dirname "$0")"

echo "== tests"
cargo test --release

echo "== compile checks"
./compile_checks/run.sh . compile_checks

echo "== miri"
cargo +"${MIRI_TOOLCHAIN:-nightly}" miri test --test miri
