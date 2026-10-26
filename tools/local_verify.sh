#!/usr/bin/env bash
# Runs the verifier's checks natively (no Docker, no Miri) against a source
# directory. Usage: tools/local_verify.sh <task dir> <src dir> [work dir]
# Exit status: 0 if every check passes.
set -uo pipefail
task=$(cd "$1" && pwd)
src=$(cd "$2" && pwd)
work=${3:-$(mktemp -d)}
mkdir -p "$work"
work=$(cd "$work" && pwd)
rm -rf "$work/app"
cp -r "$task/environment/repo" "$work/app"
rm -rf "$work/app/src"
cp -r "$src" "$work/app/src"
mkdir -p "$work/app/src/routes"
cp "$task/tests/pristine/api.rs" "$work/app/src/api.rs"
cp "$task/tests/pristine/routes/model.rs" "$work/app/src/routes/model.rs"
cp -r "$task/tests/hidden/." "$work/app/tests/"
checks="$work/checks"
rm -rf "$checks" && mkdir -p "$checks"
cp "$work/app/compile_checks/"*.rs "$task/tests/compile_checks/"*.rs "$checks/"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$work/target}"
cd "$work/app"
failed=0
for file in tests/*.rs; do
    name=$(basename "$file" .rs)
    if cargo test --release --offline --test "$name" >"$work/test_$name.log" 2>&1; then
        echo "pass  test_$name"
    else
        echo "FAIL  test_$name"
        failed=$((failed + 1))
    fi
done
if ./compile_checks/run.sh . "$checks" >"$work/compile_checks.log" 2>&1; then
    echo "pass  compile_checks"
else
    echo "FAIL  compile_checks"
    failed=$((failed + 1))
fi
echo "failed: $failed"
[ "$failed" -eq 0 ]
