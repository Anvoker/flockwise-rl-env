#!/usr/bin/env bash
# Harbor smoke test for a task: the reference solution must get reward 1 and
# an agent that does nothing must get reward 0. Rebuilds the image first.
#
# Usage: tools/harbor_smoke.sh <task dir> [jobs dir]
#
# Deletes no files. Harbor removes the containers it started when each run ends.
set -uo pipefail
task=$1
jobs=${2:-jobs}
export PYTHONUTF8=1 PYTHONIOENCODING=utf-8

run() { # agent
    uvx --from harbor harbor run -p "$task" -a "$1" -o "$jobs" -y --force-build 2>&1 \
        | sed 's/\x1b\[[0-9;]*m//g' | grep -E 'Mean:|Exception|Error' | tail -3
}

echo "== oracle (expect 1.0)"
run oracle
echo "== nop (expect 0.0)"
run nop
