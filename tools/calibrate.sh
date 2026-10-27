#!/usr/bin/env bash
# Measures how often the reference solution (or another valid solution)
# fails each test by bad luck. Builds the full suite against it, then runs
# every test binary over many disjoint seed ranges.
#
# Usage: tools/calibrate.sh <task dir> <runs> [parallel jobs] [work dir] [src dir]
# Set CALIBRATE_ONLY to a space-separated list of test files (such as
# "h_routes h_borrowed") to calibrate only those. Tests are deterministic per
# seed, so after changing some tests the others needn't run again.
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
    case " ${CALIBRATE_ONLY:-$name} " in *" $name "*) ;; *) continue ;; esac
    # A run fails if its binary exits non-zero: failed tests, but also
    # crashes and timeouts. Tests run with the harness's default threading,
    # as in the verifier.
    seq 1 "$runs" | xargs -P "$jobs" -I{} sh -c \
        "out=\$(FLOCKWISE_SEED_BASE=\$(( {} * 10 )) timeout -k 10 ${TEST_TIMEOUT:-180} '$bin' 2>&1); s=\$?;
         [ \$s -eq 0 ] || { echo \"\$out\" | grep -E '^test .* FAILED$'; echo \"run failed: exit \$s\"; }" \
        >"$out/$name.txt"
    total=$(grep -c '^run failed' "$out/$name.txt" || true)
    echo "$name: $total failures in $runs runs"
    sort "$out/$name.txt" | uniq -c | sed 's/^/    /'
done
