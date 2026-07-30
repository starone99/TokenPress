# Coverage gate: fails unless line coverage is 100%.
# The binary entry point (main.rs) is a thin uninstrumentable wrapper and is excluded.
cargo llvm-cov --workspace --fail-under-lines 100 --ignore-filename-regex 'src[/\\]main\.rs' --summary-only @args
exit $LASTEXITCODE
