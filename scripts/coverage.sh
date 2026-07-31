#!/usr/bin/env sh
# Coverage gate: fails unless line coverage is 100%. main.rs (thin entry) excluded.
exec cargo llvm-cov --workspace --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs' --summary-only "$@"
