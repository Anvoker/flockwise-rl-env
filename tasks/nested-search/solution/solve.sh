#!/usr/bin/env bash
# Reference solution: replace the library's source with the reference.
set -euo pipefail

rm -rf /app/src
cp -r /solution/src /app/src
