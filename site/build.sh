#!/usr/bin/env bash
# Builds the wasm bundle the demo page loads.
#
# Output lands in site/pkg/ (gitignored — a build artifact, never committed).
# The wasm-bindgen CLI version is derived from Cargo.lock, so the generated
# JS glue can never disagree with the `wasm-bindgen` crate the wasm blob was
# compiled against. Re-running is cheap: an already-downloaded CLI of the
# right version is reused.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
out_dir="$script_dir/pkg"
tools_dir="$script_dir/.tools"

# rustup picks the toolchain from the *current directory*, so run everything
# from the workspace root — otherwise `rust-toolchain.toml` is ignored and the
# build fails on a host whose default toolchain is older than the pin.
cd "$repo_root"

# The exact `wasm-bindgen` version the workspace links against. Cargo.lock
# lists the version on the line after the package name, so read that pair
# rather than the first `version =` in the file.
version="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[",]/, "", $3); print $3; exit }' \
    "$repo_root/Cargo.lock")"
if [ -z "$version" ]; then
    echo "error: no wasm-bindgen version in $repo_root/Cargo.lock" >&2
    exit 1
fi
echo "wasm-bindgen: $version"

install_dir="$tools_dir/wasm-bindgen-$version"
cli="$install_dir/wasm-bindgen"

# 1. The prebuilt release binary (fast path, no compilation).
download_cli() {
    local triple asset url tmp
    case "$(uname -s)/$(uname -m)" in
        Linux/x86_64) triple="x86_64-unknown-linux-musl" ;;
        Linux/aarch64) triple="aarch64-unknown-linux-gnu" ;;
        Darwin/x86_64) triple="x86_64-apple-darwin" ;;
        Darwin/arm64) triple="aarch64-apple-darwin" ;;
        *) return 1 ;;
    esac
    asset="wasm-bindgen-$version-$triple"
    url="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$version/$asset.tar.gz"
    tmp="$(mktemp -d)"
    # Unpack into a scratch directory and move into place only on success, so
    # an interrupted download never leaves a half-installed CLI that the
    # existence check would then skip.
    if curl -fL --silent --show-error --output "$tmp/cli.tar.gz" "$url" &&
        tar -xzf "$tmp/cli.tar.gz" -C "$tmp" &&
        [ -x "$tmp/$asset/wasm-bindgen" ]; then
        mkdir -p "$install_dir"
        mv "$tmp/$asset/wasm-bindgen" "$cli"
        rm -rf "$tmp"
        return 0
    fi
    rm -rf "$tmp"
    return 1
}

# 2. Fallback: build the CLI from crates.io at the same pinned version.
build_cli() {
    cargo install wasm-bindgen-cli --version "$version" --locked --root "$install_dir" >&2 &&
        mv "$install_dir/bin/wasm-bindgen" "$cli"
}

if [ ! -x "$cli" ]; then
    echo "installing wasm-bindgen-cli $version into $install_dir"
    if ! download_cli; then
        echo "prebuilt wasm-bindgen-cli unavailable; falling back to cargo install" >&2
        build_cli
    fi
fi
"$cli" --version

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p tokenpress-wasm --release --target wasm32-unknown-unknown

wasm="$repo_root/target/wasm32-unknown-unknown/release/tokenpress_wasm.wasm"
rm -rf "$out_dir"
"$cli" --target web --no-typescript --out-dir "$out_dir" "$wasm"

echo "bundle written to $out_dir"
ls -l "$out_dir"
