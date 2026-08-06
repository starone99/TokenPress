#!/bin/sh
# TokenPress installer.
#
#   curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
#
# Downloads the release archive for this host from GitHub Releases, checks it
# against the release's SHA256SUMS, and installs the binary. Nothing is
# executed from the archive, and the checksum is verified before anything is
# extracted.
#
# Environment:
#   TOKENPRESS_VERSION   tag to install (default: the latest release)
#   TOKENPRESS_BIN_DIR   install directory (default: $HOME/.local/bin)
set -eu

REPO="starone99/TokenPress"
BIN_DIR="${TOKENPRESS_BIN_DIR:-$HOME/.local/bin}"

die() { printf '\033[31merror\033[0m: %s\n' "$1" >&2; exit 1; }
info() { printf '%s\n' "$1" >&2; }

need() { command -v "$1" >/dev/null 2>&1 || die "this installer needs \`$1\` on PATH"; }
need uname
need tar

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
    fetch_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
    fetch_stdout() { wget -qO- "$1"; }
else
    die "this installer needs either \`curl\` or \`wget\` on PATH"
fi

# --- which archive does this host want? ------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
    Linux-x86_64)             target="x86_64-unknown-linux-gnu" ;;
    Darwin-arm64|Darwin-aarch64) target="aarch64-apple-darwin" ;;
    Darwin-x86_64)            target="x86_64-apple-darwin" ;;
    *)
        die "no prebuilt binary for $os-$arch.
Build from source instead:
  cargo install --git https://github.com/$REPO tokenpress-cli"
        ;;
esac

# --- which version? --------------------------------------------------------
version="${TOKENPRESS_VERSION:-}"
if [ -z "$version" ]; then
    info "resolving the latest release..."
    version="$(fetch_stdout "https://api.github.com/repos/$REPO/releases/latest" |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
    [ -n "$version" ] || die "could not resolve the latest release of $REPO.
There may not be one yet — build from source instead:
  cargo install --git https://github.com/$REPO tokenpress-cli"
fi

archive="tokenpress-$version-$target.tar.gz"
base="https://github.com/$REPO/releases/download/$version"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

info "downloading $archive ..."
fetch "$base/$archive" "$tmp/$archive" ||
    die "download failed: $base/$archive"

# --- verify before extracting ----------------------------------------------
if fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" 2>/dev/null; then
    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$tmp/$archive" | cut -d' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$tmp/$archive" | cut -d' ' -f1)"
    else
        actual=""
    fi
    if [ -n "$actual" ]; then
        expected="$(grep " \*\{0,1\}$archive\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)"
        [ -n "$expected" ] || die "$archive is not listed in the release's SHA256SUMS"
        [ "$actual" = "$expected" ] ||
            die "checksum mismatch for $archive
  expected $expected
  actual   $actual
Nothing was installed."
        info "checksum ok"
    else
        info "warning: no sha256sum/shasum on PATH — checksum NOT verified"
    fi
else
    die "could not download SHA256SUMS for $version; refusing to install unverified"
fi

# --- install ---------------------------------------------------------------
tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 755 "$tmp/tokenpress-$version-$target/tokenpress" "$BIN_DIR/tokenpress" 2>/dev/null ||
    { cp "$tmp/tokenpress-$version-$target/tokenpress" "$BIN_DIR/tokenpress" &&
      chmod 755 "$BIN_DIR/tokenpress"; }

info "installed tokenpress $version to $BIN_DIR/tokenpress"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) info "note: $BIN_DIR is not on your PATH — add it to your shell profile" ;;
esac

"$BIN_DIR/tokenpress" --version
