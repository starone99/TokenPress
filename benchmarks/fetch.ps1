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
