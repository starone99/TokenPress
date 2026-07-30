# Fetches the benchmark corpus at pinned versions.
# The corpus lives in benchmarks/corpus/ and is gitignored (license/size).
$ErrorActionPreference = "Stop"
$corpus = Join-Path $PSScriptRoot "corpus"
New-Item -ItemType Directory -Force $corpus | Out-Null

if (-not (Test-Path (Join-Path $corpus "requests"))) {
    git clone --quiet --depth 1 --branch v2.32.3 https://github.com/psf/requests (Join-Path $corpus "requests")
}
if (-not (Test-Path (Join-Path $corpus "ripgrep"))) {
    git clone --quiet --depth 1 --branch 14.1.1 https://github.com/BurntSushi/ripgrep (Join-Path $corpus "ripgrep")
}

"requests: $(git -C (Join-Path $corpus 'requests') rev-parse HEAD)"
"ripgrep:  $(git -C (Join-Path $corpus 'ripgrep') rev-parse HEAD)"

# Open-model tokenizer files (revision-pinned), for --tokenizer hf:/kimi:
$toks = Join-Path $PSScriptRoot "tokenizers"
New-Item -ItemType Directory -Force $toks | Out-Null
$downloads = @(
    @{ name = "qwen3.6.json"; url = "https://huggingface.co/Qwen/Qwen3.6-35B-A3B/resolve/995ad96eacd98c81ed38be0c5b274b04031597b0/tokenizer.json" },
    @{ name = "glm-5.2.json"; url = "https://huggingface.co/zai-org/GLM-5.2/resolve/b4734de4facf877f85769a911abafc5283eab3d9/tokenizer.json" },
    @{ name = "kimi-k3.tiktoken"; url = "https://huggingface.co/moonshotai/Kimi-K3/resolve/9f62e4e9fffbd0a83ddd60e1c209d828994b3569/tiktoken.model" }
)
foreach ($d in $downloads) {
    $dest = Join-Path $toks $d.name
    if (-not (Test-Path $dest)) {
        Invoke-WebRequest -Uri $d.url -OutFile $dest
    }
    "$($d.name): $((Get-Item $dest).Length) bytes"
}
