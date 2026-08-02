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

# Well-known projects, pinned to the exact commits measured in RESULTS.md.
$known = @(
    @{ name = "django";       url = "https://github.com/django/django";            sha = "50d706d0aebcc2d073c8d034b6e22fc98fad49f2" },
    @{ name = "fastapi";      url = "https://github.com/fastapi/fastapi";          sha = "95f8322ee1dcda7ceace7b1c4f6c9915b36d748f" },
    @{ name = "tokio";        url = "https://github.com/tokio-rs/tokio";           sha = "adc2ae7af2caaea83985fbdfbc7884c159c486f2" },
    @{ name = "langchain";    url = "https://github.com/langchain-ai/langchain";   sha = "a1a1ad3bb3eb6cf7680b39ff0fb37f7150393a25" },
    @{ name = "transformers"; url = "https://github.com/huggingface/transformers"; sha = "71c6f699ac9b3f8fc42a6a3e9dc59034c349a678" },
    @{ name = "uv";           url = "https://github.com/astral-sh/uv";             sha = "be765050837d81badb20e1f70eec62146c586902" },
    @{ name = "express";      url = "https://github.com/expressjs/express";        sha = "dbac741a49a5a64336b70c06e85c2e2706e36336" }
)
foreach ($k in $known) {
    $dest = Join-Path $corpus $k.name
    if (-not (Test-Path $dest)) {
        git init -q $dest
        git -C $dest config core.longpaths true
        git -C $dest remote add origin $k.url
        git -C $dest fetch -q --depth 1 origin $k.sha
        git -C $dest checkout -q FETCH_HEAD
    }
    "$($k.name): $(git -C $dest rev-parse HEAD)"
}

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
