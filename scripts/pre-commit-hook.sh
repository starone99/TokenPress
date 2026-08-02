#!/usr/bin/env sh
# Entry point for the hooks declared in `.pre-commit-hooks.yaml`.
#
# pre-commit clones this repository into its own cache and runs this script
# from the *consuming* repository's working directory. The CLI is therefore
# built inside the clone -- `cargo` has to run there to pick up
# `rust-toolchain.toml` -- while the file arguments stay relative to the
# caller's directory, so the built binary is executed without changing back.
#
# Usage: scripts/pre-commit-hook.sh <tokenpress subcommand> [args...] [files...]
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

target_dir="$repo_dir/target"

# `TOKENPRESS_NO_RUBY=1` builds without the default-on `ruby` cargo feature,
# which drops the Ruby backend and with it the libclang + C compiler build
# prerequisite. Ruby files are then unsupported paths like any other.
no_ruby=''
if [ "${TOKENPRESS_NO_RUBY:-}" = 1 ]; then
    no_ruby='--no-default-features'
fi

# Build output belongs on stderr: hook stdout stays the CLI's own report.
# `--target-dir` is explicit so an inherited `CARGO_TARGET_DIR` cannot move the
# binary away from where it is executed from below.
# `$no_ruby` is deliberately unquoted: empty must expand to no argument at all.
(
    cd "$repo_dir" &&
        cargo build --release --locked --quiet $no_ruby \
            -p tokenpress-cli --target-dir "$target_dir"
) 1>&2

exec "$target_dir/release/tokenpress" "$@"
