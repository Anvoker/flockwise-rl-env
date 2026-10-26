#!/usr/bin/env bash
# Measures how often the reference solution (or another valid solution)
# fails each test by bad luck. Builds the full suite against it, then runs
# every test binary over many disjoint seed ranges.
#
# Usage: tools/calibrate.sh <task dir> <runs> [parallel jobs] [work dir] [src dir]
set -euo pipefail
task=$(cd "$1" && pwd)
runs=$2
jobs=${3:-4}
work=${4:-$(mktemp -d)}
mkdir -p "$work"
work=$(cd "$work" && pwd)
# "$work/calibration" is deleted below: refuse an empty or root work dir.
case "$work" in
    "" | "/" | "/c" | "/c/") echo "refusing to use work dir '$work'" >&2; exit 1 ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
src=${5:-$task/solution/src}

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$work/target}"
bash "$here/local_verify.sh" "$task" "$src" "$work/verify" >/dev/null || {
    echo "$src doesn't pass the verifier; fix that first" >&2
    exit 1
}

cd "$work/verify/app"
bins=$(cargo test --release --offline --no-run --message-format=json 2>/dev/null \
    | grep -o '"executable":"[^"]*"' | cut -d'"' -f4 | grep -v '/flockwise-' || true)

out="$work/calibration"
rm -rf "$out" && mkdir -p "$out"
for bin in $bins; do
    name=$(basename "$bin" | sed 's/-[0-9a-f]*\(\.exe\)\{0,1\}$//')
    seq 1 "$runs" | xargs -P "$jobs" -I{} sh -c \
        "FLOCKWISE_SEED_BASE=\$(( {} * 10 )) '$bin' --test-threads=1 2>&1 | grep -E '^test .* FAILED$' || true" \
        >"$out/$name.txt"
    total=$(grep -c 'FAILED' "$out/$name.txt" || true)
    echo "$name: $total failures in $runs runs"
    sort "$out/$name.txt" | uniq -c | sed 's/^/    /'
done
