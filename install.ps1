# TokenPress installer for Windows.
#
#   irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
#
# Downloads the release archive for this host from GitHub Releases, checks it
# against the release's SHA256SUMS, and installs the binary. The checksum is
# verified before anything is extracted.
#
# Environment:
#   TOKENPRESS_VERSION   tag to install (default: the latest release)
#   TOKENPRESS_BIN_DIR   install directory (default: %LOCALAPPDATA%\TokenPress\bin)
$ErrorActionPreference = 'Stop'

$repo = 'starone99/TokenPress'
$binDir = if ($env:TOKENPRESS_BIN_DIR) { $env:TOKENPRESS_BIN_DIR }
          else { Join-Path $env:LOCALAPPDATA 'TokenPress\bin' }

if ([Environment]::Is64BitOperatingSystem -eq $false) {
    throw "No prebuilt binary for 32-bit Windows. Build from source instead: cargo install --git https://github.com/$repo tokenpress-cli"
}
$target = 'x86_64-pc-windows-msvc'

# --- which version? --------------------------------------------------------
$version = $env:TOKENPRESS_VERSION
if (-not $version) {
    Write-Host 'Resolving the latest release...'
    try {
        $rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
        $version = $rel.tag_name
    } catch {
        throw "Could not resolve the latest release of $repo. There may not be one yet -- build from source instead: cargo install --git https://github.com/$repo tokenpress-cli"
    }
}

$archive = "tokenpress-$version-$target.zip"
$base = "https://github.com/$repo/releases/download/$version"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("tokenpress-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $tmp | Out-Null

try {
    Write-Host "Downloading $archive ..."
    Invoke-WebRequest -Uri "$base/$archive" -OutFile (Join-Path $tmp $archive) -UseBasicParsing

    # --- verify before extracting ------------------------------------------
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS') -UseBasicParsing
    $expected = (Get-Content (Join-Path $tmp 'SHA256SUMS') |
        Where-Object { $_ -match [regex]::Escape($archive) + '$' } |
        ForEach-Object { ($_ -split '\s+')[0] } | Select-Object -First 1)
    if (-not $expected) { throw "$archive is not listed in the release's SHA256SUMS" }

    $actual = (Get-FileHash (Join-Path $tmp $archive) -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected.ToLower()) {
        throw "Checksum mismatch for ${archive}: expected $expected, got $actual. Nothing was installed."
    }
    Write-Host 'Checksum ok'

    # --- install ------------------------------------------------------------
    Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force $binDir | Out-Null
    Copy-Item (Join-Path $tmp "tokenpress-$version-$target\tokenpress.exe") `
              (Join-Path $binDir 'tokenpress.exe') -Force

    Write-Host "Installed tokenpress $version to $binDir\tokenpress.exe"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$binDir*") {
        Write-Host "Note: $binDir is not on your PATH. Add it with:"
        Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$binDir`", 'User')"
    }

    & (Join-Path $binDir 'tokenpress.exe') --version
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
