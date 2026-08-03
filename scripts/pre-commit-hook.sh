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

# `TOKENPRESS_NO_RUBY=1` and `TOKENPRESS_NO_GO=1` build without the default-on
# `ruby` / `go` cargo features. Dropping `ruby` drops the libclang + C compiler
# prerequisite that `ruby-prism-sys` imposes; dropping `go` drops the C
# compiler that the tree-sitter runtime and the Go grammar need. Files of a
# dropped backend are then unsupported paths like any other.
#
# The two are independent, so neither variable can simply mean
# `--no-default-features`: that is the both-off build. What is left on has to
# be named explicitly, which is what the feature list below does.
features=''
if [ "${TOKENPRESS_NO_RUBY:-}" != 1 ]; then
    features='ruby'
fi
if [ "${TOKENPRESS_NO_GO:-}" != 1 ]; then
    features="${features:+$features,}go"
fi
case "$features" in
    'ruby,go') build_flags='' ;;
    '') build_flags='--no-default-features' ;;
    *) build_flags="--no-default-features --features $features" ;;
esac

# Build output belongs on stderr: hook stdout stays the CLI's own report.
# `--target-dir` is explicit so an inherited `CARGO_TARGET_DIR` cannot move the
# binary away from where it is executed from below.
# `$build_flags` is deliberately unquoted: it has to expand to no argument at
# all when empty, and to its two or three separate words when it is not.
(
    cd "$repo_dir" &&
        cargo build --release --locked --quiet $build_flags \
            -p tokenpress-cli --target-dir "$target_dir"
) 1>&2

exec "$target_dir/release/tokenpress" "$@"
