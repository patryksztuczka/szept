#!/bin/sh
# The checks to run before every commit. Pass extra cargo flags through, e.g.
# ./scripts/check.sh --features runtime-shaders
set -eu

cargo fmt --check
cargo clippy --all-targets "$@" -- -D warnings
cargo test "$@"
