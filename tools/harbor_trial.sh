#!/usr/bin/env bash
# One agent trial through Harbor with Claude Code on a Claude subscription.
# The OAuth token (from `claude setup-token`) is read from a file into the
# environment, so it never appears on a command line or in the output.
#
# Usage: tools/harbor_trial.sh <task dir> <model> [token file] [jobs dir]
#   e.g. tools/harbor_trial.sh tasks/nested-search anthropic/claude-sonnet-5-5
#
# Deletes no files. Harbor removes the containers it started when the run ends.
set -uo pipefail
task=$1
model=$2
token_file=${3:-$HOME/.secrets/claude-oauth-token.txt}
jobs=${4:-jobs}

if [ ! -s "$token_file" ]; then
    echo "no token in $token_file" >&2
    exit 1
fi
CLAUDE_CODE_OAUTH_TOKEN=$(tr -d '\r\n\t ' <"$token_file")
export CLAUDE_CODE_OAUTH_TOKEN
export PYTHONUTF8=1 PYTHONIOENCODING=utf-8

uvx --from harbor harbor run -p "$task" -a claude-code -m "$model" -o "$jobs" -y 2>&1 \
    | sed 's/\x1b\[[0-9;]*m//g' | grep -v '^\s*$' | tail -20
