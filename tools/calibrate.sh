#!/usr/bin/env bash
# Measures how often the reference solution fails each test by bad luck.
# Builds the full suite against the reference, then runs every test binary
# over many disjoint seed ranges.
#
# Usage: tools/calibrate.sh <task dir> <runs> [parallel jobs] [work dir]
set -euo pipefail
task=$(cd "$1" && pwd)
runs=$2
jobs=${3:-4}
work=${4:-$(mktemp -d)}
here=$(cd "$(dirname "$0")" && pwd)

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$work/target}"
bash "$here/local_verify.sh" "$task" "$task/solution/src" "$work/verify" >/dev/null || {
    echo "the reference doesn't pass the verifier; fix that first" >&2
    exit 1
}

cd "$work/verify/app"
bins=$(cargo test --release --offline --no-run --message-format=json 2>/dev/null \
    | grep -o '"executable":"[^"]*"' | cut -d'"' -f4 | grep -v '/nestswarm-' || true)

out="$work/calibration"
rm -rf "$out" && mkdir -p "$out"
for bin in $bins; do
    name=$(basename "$bin" | sed 's/-[0-9a-f]*\(\.exe\)\{0,1\}$//')
    seq 1 "$runs" | xargs -P "$jobs" -I{} sh -c \
        "NESTSWARM_SEED_BASE=\$(( {} * 10 )) '$bin' --test-threads=1 2>&1 | grep -E '^test .* FAILED$' || true" \
        >"$out/$name.txt"
    total=$(grep -c 'FAILED' "$out/$name.txt" || true)
    echo "$name: $total failures in $runs runs"
    sort "$out/$name.txt" | uniq -c | sed 's/^/    /'
done
