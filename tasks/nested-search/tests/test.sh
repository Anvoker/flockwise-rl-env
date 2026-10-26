#!/usr/bin/env bash
# Verifier. Runs in a fresh copy of the environment: /app is the starting
# repository, except /app/src, which holds the agent's work.
set -uo pipefail

APP=/app
LOGS=/logs/verifier
mkdir -p "$LOGS"
cd "$APP"

# The public traits are part of the contract: restore them.
cp /tests/pristine/api.rs src/api.rs

# Hidden tests and compile checks go next to the visible ones.
cp -r /tests/hidden/. tests/
checks=$(mktemp -d)
cp compile_checks/*.rs /tests/compile_checks/*.rs "$checks/"

passed=0
failed=0
results="{"

record() { # name status
    if [ "$2" -eq 0 ]; then
        passed=$((passed + 1))
        echo "pass  $1"
    else
        failed=$((failed + 1))
        echo "FAIL  $1 (log: $LOGS/$1.log)"
    fi
    results="$results\"$1\": $([ "$2" -eq 0 ] && echo 1 || echo 0), "
}

# Each test file is compiled on its own, so one that doesn't compile can't
# hide the results of the others.
for file in tests/*.rs; do
    name=$(basename "$file" .rs)
    cargo test --release --offline --test "$name" >"$LOGS/test_$name.log" 2>&1
    record "test_$name" $?
done

./compile_checks/run.sh . "$checks" >"$LOGS/compile_checks.log" 2>&1
record compile_checks $?

cargo +"$MIRI_TOOLCHAIN" miri test --offline --test miri >"$LOGS/miri.log" 2>&1
record miri $?

echo "${results%, }}" >"$LOGS/results.json"
echo "$passed passed, $failed failed"

if [ "$failed" -eq 0 ]; then
    echo 1 >"$LOGS/reward.txt"
else
    echo 0 >"$LOGS/reward.txt"
fi
