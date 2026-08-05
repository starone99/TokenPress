#!/usr/bin/env bash
# Fetches the benchmark corpus at pinned versions.
# The corpus lives in benchmarks/corpus/ and is gitignored (license/size).
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
corpus="$script_dir/corpus"
mkdir -p "$corpus"

if [ ! -e "$corpus/requests" ]; then
    git clone --quiet --depth 1 --branch v2.32.3 https://github.com/psf/requests "$corpus/requests"
fi
if [ ! -e "$corpus/ripgrep" ]; then
    git clone --quiet --depth 1 --branch 14.1.1 https://github.com/BurntSushi/ripgrep "$corpus/ripgrep"
fi

echo "requests: $(git -C "$corpus/requests" rev-parse HEAD)"
echo "ripgrep:  $(git -C "$corpus/ripgrep" rev-parse HEAD)"

# Well-known projects, pinned to the exact commits measured in RESULTS.md.
# Five of them are pinned at a tagged release rather than a snapshot: express
# `dbac741a` is tag v5.2.1, rack `e1f22fdb` is tag v3.2.6, gin `6ad6205e` is
# tag v1.11.0, commons-lang `29ccc766` is tag rel/commons-lang-3.17.0 and
# csvhelper `5dad8b8b` is tag 33.1.0. Those are the tags verify-upstream.sh
# clones, and it asserts the same SHAs after the clone.
known=(
    "django|https://github.com/django/django|50d706d0aebcc2d073c8d034b6e22fc98fad49f2"
    "fastapi|https://github.com/fastapi/fastapi|95f8322ee1dcda7ceace7b1c4f6c9915b36d748f"
    "tokio|https://github.com/tokio-rs/tokio|adc2ae7af2caaea83985fbdfbc7884c159c486f2"
    "langchain|https://github.com/langchain-ai/langchain|a1a1ad3bb3eb6cf7680b39ff0fb37f7150393a25"
    "transformers|https://github.com/huggingface/transformers|71c6f699ac9b3f8fc42a6a3e9dc59034c349a678"
    "uv|https://github.com/astral-sh/uv|be765050837d81badb20e1f70eec62146c586902"
    "express|https://github.com/expressjs/express|dbac741a49a5a64336b70c06e85c2e2706e36336"
    "rack|https://github.com/rack/rack|e1f22fdbe99afd2126b6fbf05bb12399359574b7"
    "gin|https://github.com/gin-gonic/gin|6ad6205e9c94a4b8a320219e28c37c29d22a7a2c"
    "commons-lang|https://github.com/apache/commons-lang|29ccc7665f3bc5d84155a3092ab2209a053324e6"
    "csvhelper|https://github.com/JoshClose/CsvHelper|5dad8b8b1d8b074f8353cfd482e939db788a8927"
)
for entry in "${known[@]}"; do
    IFS='|' read -r name url sha <<< "$entry"
    dest="$corpus/$name"
    if [ ! -e "$dest" ]; then
        git init -q "$dest"
        # No-op outside Windows, but kept so the script behaves identically
        # when run from Git Bash / WSL on a Windows checkout.
        git -C "$dest" config core.longpaths true
        git -C "$dest" remote add origin "$url"
        git -C "$dest" fetch -q --depth 1 origin "$sha"
        git -C "$dest" checkout -q FETCH_HEAD
    fi
    echo "$name: $(git -C "$dest" rev-parse HEAD)"
done

# Open-model tokenizer files (revision-pinned), for --tokenizer hf:/kimi:
toks="$script_dir/tokenizers"
mkdir -p "$toks"
downloads=(
    "qwen3.6.json|https://huggingface.co/Qwen/Qwen3.6-35B-A3B/resolve/995ad96eacd98c81ed38be0c5b274b04031597b0/tokenizer.json"
    "glm-5.2.json|https://huggingface.co/zai-org/GLM-5.2/resolve/b4734de4facf877f85769a911abafc5283eab3d9/tokenizer.json"
    "kimi-k3.tiktoken|https://huggingface.co/moonshotai/Kimi-K3/resolve/9f62e4e9fffbd0a83ddd60e1c209d828994b3569/tiktoken.model"
    # Pinned at the google/gemma-4-31B *base* repo, like the three above.
    # Gemma 2 and Gemma 3 are gated (`gated: manual` — license acceptance plus
    # an auth token); Gemma 4 is not, so this needs no HF_TOKEN and no
    # community mirror. Only the 31B base is pinned, but the other Gemma 4
    # base repos (12B, 26B-A4B, E4B) serve a byte-identical tokenizer.json
    # (same 32,170,070 bytes, same LFS oid); `-it` does not.
    "gemma-4.json|https://huggingface.co/google/gemma-4-31B/resolve/5bbc2fb1c1b2c611d06e3d9f23c170ba21659d89/tokenizer.json"
)
for entry in "${downloads[@]}"; do
    IFS='|' read -r name url <<< "$entry"
    dest="$toks/$name"
    if [ ! -e "$dest" ]; then
        # Download to a temporary file and move it into place only on success,
        # so a failed request never leaves a truncated file behind (which would
        # then be skipped by the existence check on the next run).
        tmp="$(mktemp "$dest.XXXXXX")"
        if curl -fL --silent --show-error --output "$tmp" "$url"; then
            # mktemp creates 0600; make it world-readable like a normal download.
            chmod 644 "$tmp"
            mv "$tmp" "$dest"
        else
            rm -f "$tmp"
            exit 1
        fi
    fi
    echo "$name: $(wc -c < "$dest" | tr -d ' ') bytes"
done
