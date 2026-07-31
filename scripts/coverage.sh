#!/usr/bin/env sh
# Coverage gate: fails unless line coverage is 100%.
# Sole exclusion: the CLI's thin, uninstrumentable entry point. The regex is
# anchored on the crate directory so a main.rs in any other crate still counts.
exec cargo llvm-cov --workspace --fail-under-lines 100 --ignore-filename-regex 'tokenpress-cli/src/main\.rs' --summary-only "$@"
