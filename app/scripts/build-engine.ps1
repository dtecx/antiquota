# Windows: build the engine as a self-contained folder (app\engine-dist\antiquotad) with PyInstaller.
# pywinpty gives agy the console it needs to read the Google sign-in code.
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent $here
$venv = Join-Path $here ".venv"
if (-not (Test-Path (Join-Path $venv "Scripts\python.exe"))) { python -m venv $venv }
$py = Join-Path $venv "Scripts\python.exe"
& $py -m pip install --quiet --upgrade pip pyinstaller pywinpty
Push-Location $root
& (Join-Path $venv "Scripts\pyinstaller.exe") --noconfirm --clean --onedir --name antiquotad `
  --distpath (Join-Path $here "engine-dist") --workpath (Join-Path $here ".build") --specpath (Join-Path $here ".build") `
  --paths $root --add-data "$root\antiquota\ui;antiquota\ui" --collect-submodules antiquota --hidden-import winpty `
  (Join-Path $here "engine\antiquotad.py")
Pop-Location
Write-Host "engine: $here\engine-dist\antiquotad"
