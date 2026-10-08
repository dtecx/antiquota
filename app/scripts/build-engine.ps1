# Bundle the Rust engine and the same production UI used by browsers.
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent $here
Push-Location $root
try {
    & cargo build --release --locked --jobs 2 -p antiquota-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust engine build failed" }
    & pnpm --dir ui install --frozen-lockfile
    if ($LASTEXITCODE -ne 0) { throw "UI dependency installation failed" }
    & pnpm --dir ui build
    if ($LASTEXITCODE -ne 0) { throw "UI build failed" }
    $bin = Join-Path $here "engine-dist/bin"
    New-Item -ItemType Directory -Force -Path $bin | Out-Null
    Copy-Item (Join-Path $root "target/release/antiquota.exe") (Join-Path $bin "antiquota.exe") -Force
    $ui = Join-Path $here "engine-dist/ui"
    if (Test-Path $ui) { Remove-Item $ui -Recurse -Force }
    Copy-Item (Join-Path $root "ui/dist") $ui -Recurse
    & (Join-Path $bin "antiquota.exe") --help
    if ($LASTEXITCODE -ne 0) { throw "Bundled engine does not start" }
} finally { Pop-Location }
