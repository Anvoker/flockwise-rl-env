#!/usr/bin/env bash
# Compile checks: every case must compile in its "CASE" form and fail to
# compile in its "FAIL" form. Usage: run.sh <crate dir> <cases dir>
set -u
crate=$(cd "$1" && (pwd -W 2>/dev/null || pwd))
cases=$(cd "$2" && pwd)
work=$(mktemp -d)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$work/target}"
status=0

make_crate() { # name variant
    local dir="$work/$1-$2"
    mkdir -p "$dir/src"
    cat > "$dir/Cargo.toml" <<TOML
[package]
name = "check-$1-$2"
version = "0.0.0"
edition = "2024"

[dependencies]
flockwise = { path = "$crate" }
rand = { version = "0.9.2", features = ["std_rng"] }
TOML
    {
        cat "$cases/prelude.rs"
        if [ "$2" = pass ]; then
            sed -e 's#^\(\s*\)//CASE #\1#' -e '/^\s*\/\/FAIL /d' "$cases/$1.rs"
        else
            sed -e 's#^\(\s*\)//FAIL #\1#' -e '/^\s*\/\/CASE /d' "$cases/$1.rs"
        fi
    } > "$dir/src/main.rs"
    echo "$dir"
}

for f in "$cases"/*.rs; do
    name=$(basename "$f" .rs)
    [ "$name" = prelude ] && continue
    for variant in pass fail; do
        dir=$(make_crate "$name" "$variant")
        if (cd "$dir" && cargo check --quiet --offline >"$dir/out.txt" 2>&1); then ok=pass; else ok=fail; fi
        if [ "$ok" = "$variant" ]; then
            echo "ok   $name ($variant)"
        else
            echo "FAIL $name: the $variant form should $( [ $variant = pass ] && echo compile || echo 'fail to compile' )"
            sed -n '1,30p' "$dir/out.txt"
            status=1
        fi
    done
done
exit $status
