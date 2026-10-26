#!/usr/bin/env bash
# Verifier. Runs in a fresh copy of the environment: /app is the starting
# repository, except /app/src, which holds the agent's work.
set -uo pipefail

APP=/app
LOGS=/logs/verifier
mkdir -p "$LOGS"
cd "$APP"

# The public traits and the route model are part of the contract: restore them.
mkdir -p src/routes
cp /tests/pristine/api.rs src/api.rs
cp /tests/pristine/routes/model.rs src/routes/model.rs

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

# Builds one test file, then runs its binary with a time limit, so a
# deadlocked solution fails that file instead of stalling the verifier.
# Usage: run_test <name> [test binary args...]
run_test() {
    local out status exe
    out=$(cargo test --release --offline --test "$1" --no-run 2>&1)
    status=$?
    echo "$out"
    [ "$status" -eq 0 ] || return "$status"
    exe=$(echo "$out" | sed -n 's/^ *Executable .*(\(.*\))$/\1/p' | tail -1)
    [ -n "$exe" ] || return 1
    timeout -k 10 "${TEST_TIMEOUT:-180}" "$exe" "${@:2}"
}

# Each test file is compiled on its own, so one that doesn't compile can't
# hide the results of the others.
for file in tests/*.rs; do
    name=$(basename "$file" .rs)
    run_test "$name" >"$LOGS/test_$name.log" 2>&1
    record "test_$name" $?
done

./compile_checks/run.sh . "$checks" >"$LOGS/compile_checks.log" 2>&1
record compile_checks $?

# Same flags as ci.sh: crossbeam-epoch (under rayon) breaks Stacked Borrows,
# and rayon's pool threads outlive the test.
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks" timeout -k 10 900 cargo +"$MIRI_TOOLCHAIN" miri test --offline --test miri >"$LOGS/miri.log" 2>&1
record miri $?

# Reported, not scored: how much faster 8 threads are than 1.
run_test h_parallel --ignored --nocapture speedup_report >"$LOGS/speedup.log" 2>&1
speedup=$(grep -o 'SPEEDUP_8_VS_1 [0-9.]*' "$LOGS/speedup.log" | cut -d' ' -f2)
results="$results\"speedup_8_vs_1 (not scored)\": ${speedup:-null}, "

echo "${results%, }}" >"$LOGS/results.json"
echo "$passed passed, $failed failed"

if [ "$failed" -eq 0 ]; then
    echo 1 >"$LOGS/reward.txt"
else
    echo 0 >"$LOGS/reward.txt"
fi
