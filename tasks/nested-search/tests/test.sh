#!/usr/bin/env bash
# Verifier. Runs in a fresh copy of the environment: /app is the starting
# repository, except /app/src, which holds the agent's work.
#
# The agent's code is untrusted, so:
# - everything is built before any of it runs, so it can't change a test
#   before that test is compiled;
# - it runs only as an unprivileged user, who can't write to /app or /tests,
#   and whose leftover processes are killed after every step;
# - the reward is written last, by root, after removing any reward file the
#   agent's code might have left. (Harbor mounts the log directory from the
#   host, and any user can write to it.)
set -uo pipefail

APP=/app
LOGS=/logs/verifier
RUNNER=65534 # nobody
mkdir -p "$LOGS"
cd "$APP"

# The public traits and the route model are part of the contract: restore
# them. (The agent's lib.rs decides what gets compiled, so the hidden tests
# don't rely on these copies: they judge results with their own code.)
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

# Runs a command as the unprivileged user, in a directory of its own, with a
# time limit, so a deadlocked solution fails that step instead of stalling.
sandbox=$(mktemp -d)
chown "$RUNNER:$RUNNER" "$sandbox"
as_runner() {
    (cd "$sandbox" && HOME="$sandbox" timeout -k 10 "${TEST_TIMEOUT:-180}" \
        setpriv --reuid="$RUNNER" --regid="$RUNNER" --clear-groups --no-new-privs -- "$@")
}
# Kills every process the unprivileged user still has.
stop_runner() {
    setpriv --reuid="$RUNNER" --regid="$RUNNER" --clear-groups -- bash -c 'kill -KILL -1' 2>/dev/null
    return 0
}

# 1. Build. Compiling runs none of the agent's code: Cargo.toml is the
# starting one, so the agent can't add build scripts or procedural macros.
# Each test file is compiled on its own, so one that doesn't compile can't
# hide the results of the others.
declare -A exe
names=()
for file in tests/*.rs; do
    name=$(basename "$file" .rs)
    names+=("$name")
    out=$(cargo test --release --offline --test "$name" --no-run 2>&1)
    status=$?
    echo "$out" >"$LOGS/test_$name.log"
    path=$(echo "$out" | sed -n 's/^ *Executable .*(\(.*\))$/\1/p' | tail -1)
    if [ "$status" -eq 0 ] && [ -n "$path" ]; then
        # Cargo prints it relative to /app; the tests run elsewhere.
        case "$path" in /*) ;; *) path="$APP/$path" ;; esac
        exe[$name]=$path
    fi
done

# Compile checks only type-check.
./compile_checks/run.sh . "$checks" >"$LOGS/compile_checks.log" 2>&1
record compile_checks $?

# 2. Miri, as root: it builds into the target directory and uses root's
# toolchain. Miri's default isolation keeps the interpreted program away from
# the file system, the environment and other processes. Same flags as ci.sh:
# crossbeam-epoch (under rayon) breaks Stacked Borrows, and rayon's pool
# threads outlive the test.
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks" timeout -k 10 900 cargo +"$MIRI_TOOLCHAIN" miri test --offline --test miri >"$LOGS/miri.log" 2>&1
record miri $?

# 3. Run the tests, as the unprivileged user.
for name in "${names[@]}"; do
    if [ -n "${exe[$name]:-}" ]; then
        as_runner "${exe[$name]}" >>"$LOGS/test_$name.log" 2>&1
        status=$?
    else
        status=1
    fi
    stop_runner
    record "test_$name" "$status"
done

# Reported, not scored: how much faster 8 threads are than 1.
if [ -n "${exe[h_parallel]:-}" ]; then
    as_runner "${exe[h_parallel]}" --ignored --nocapture speedup_report >"$LOGS/speedup.log" 2>&1
    stop_runner
fi
speedup=$(grep -o 'SPEEDUP_8_VS_1 [0-9.]*' "$LOGS/speedup.log" 2>/dev/null | cut -d' ' -f2)
results="$results\"speedup_8_vs_1 (not scored)\": ${speedup:-null}, "

# 4. The reward. Harbor reads reward.json before reward.txt, so remove both
# before writing the one the verifier decides.
stop_runner
rm -f "$LOGS/reward.json" "$LOGS/reward.txt" "$LOGS/results.json"
echo "${results%, }}" >"$LOGS/results.json"
echo "$passed passed, $failed failed"

if [ "$failed" -eq 0 ]; then
    echo 1 >"$LOGS/reward.txt"
else
    echo 0 >"$LOGS/reward.txt"
fi
