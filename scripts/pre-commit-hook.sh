#!/usr/bin/env sh
# Entry point for the hooks declared in `.pre-commit-hooks.yaml`.
#
# pre-commit clones this repository into its own cache and runs this script
# from the *consuming* repository's working directory. The CLI is therefore
# obtained inside the clone -- a downloaded release binary is cached under its
# `target/`, and a source build has to run there to pick up
# `rust-toolchain.toml` -- while the file arguments stay relative to the
# caller's directory, so the resulting binary is executed without changing
# back.
#
# Usage: scripts/pre-commit-hook.sh <tokenpress subcommand> [args...] [files...]
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

target_dir="$repo_dir/target"

# `TOKENPRESS_NO_RUBY=1`, `TOKENPRESS_NO_GO=1`, `TOKENPRESS_NO_JAVA=1` and
# `TOKENPRESS_NO_CSHARP=1` build without the default-on `ruby` / `go` / `java`
# / `csharp` cargo features. Dropping `ruby` drops the libclang + C compiler
# prerequisite that `ruby-prism-sys` imposes; `go`, `java` and `csharp` each
# need a C compiler for the tree-sitter runtime and their own grammar, so the
# C compiler goes only once **all four** are off. Files of a dropped backend
# are then unsupported paths like any other.
#
# The four are independent, so no single variable can mean
# `--no-default-features`: that is the all-off build. What is left on has to
# be named explicitly, which is what the feature list below does. The list is
# built in the manifest's own order, so the all-on case is one literal.
features=''
if [ "${TOKENPRESS_NO_RUBY:-}" != 1 ]; then
    features='ruby'
fi
if [ "${TOKENPRESS_NO_GO:-}" != 1 ]; then
    features="${features:+$features,}go"
fi
if [ "${TOKENPRESS_NO_JAVA:-}" != 1 ]; then
    features="${features:+$features,}java"
fi
if [ "${TOKENPRESS_NO_CSHARP:-}" != 1 ]; then
    features="${features:+$features,}csharp"
fi
case "$features" in
    'ruby,go,java,csharp') build_flags='' ;;
    '') build_flags='--no-default-features' ;;
    *) build_flags="--no-default-features --features $features" ;;
esac

# --- prebuilt binary, or a source build ------------------------------------
# Compiling this workspace is not cheap -- tree-sitter grammars are C, and
# `ruby-prism-sys` runs bindgen -- so a consumer whose pin corresponds to a
# release downloads that release's binary instead of building it. Two
# conditions gate that, and both are about the binary being *the same program*
# the pin names rather than about saving time:
#
#  * The checkout is exactly at a tag. `rev:` in a consumer's config may be a
#    branch or a commit, and there is no release binary for those; falling
#    back is the only correct answer. This also keeps `check` deterministic --
#    a formatter's verdict must come from the pinned revision, never from
#    whatever release happens to be newest.
#  * Every backend is on. The `TOKENPRESS_NO_*` variables ask for a *smaller*
#    binary, and a released one has all four backends linked in, so honoring
#    them means building. (They exist to drop the libclang and C-compiler
#    prerequisites, which a download drops anyway -- but a no-Ruby build also
#    stops accepting `.rb` paths, and that difference is the consumer's to
#    choose.)
#
# `TOKENPRESS_NO_PREBUILT=1` forces the source build regardless.
version=''
if [ "${TOKENPRESS_NO_PREBUILT:-}" != 1 ] && [ "$features" = 'ruby,go,java,csharp' ]; then
    version="$(git -C "$repo_dir" describe --tags --exact-match HEAD 2>/dev/null || true)"
fi

# Hook stdout stays the CLI's own report, so every byte the installer and the
# build write goes to stderr.
bin=''
if [ -n "$version" ]; then
    prebuilt_dir="$target_dir/prebuilt/$version"
    if [ -x "$prebuilt_dir/tokenpress" ]; then
        bin="$prebuilt_dir/tokenpress"
    elif TOKENPRESS_VERSION="$version" TOKENPRESS_BIN_DIR="$prebuilt_dir" \
        sh "$repo_dir/install.sh" 1>&2; then
        bin="$prebuilt_dir/tokenpress"
    else
        # No prebuilt archive for this host (Windows and every non-x86_64
        # Linux included), a private release the runner cannot read, or a
        # failed checksum -- the installer refuses to install unverified, and
        # a refusal here must not become a refusal to run.
        rm -rf "$prebuilt_dir"
        printf 'tokenpress: no usable prebuilt binary for %s here; building from source\n' \
            "$version" >&2
    fi
fi

# `--target-dir` is explicit so an inherited `CARGO_TARGET_DIR` cannot move the
# binary away from where it is executed from below.
# `$build_flags` is deliberately unquoted: it has to expand to no argument at
# all when empty, and to its two or three separate words when it is not.
if [ -z "$bin" ]; then
    (
        cd "$repo_dir" &&
            cargo build --release --locked --quiet $build_flags \
                -p tokenpress-cli --target-dir "$target_dir"
    ) 1>&2
    bin="$target_dir/release/tokenpress"
fi

exec "$bin" "$@"
