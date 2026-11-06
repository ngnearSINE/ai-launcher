<#
  Local build. Sources stay in this folder (synced); dependencies and build output live in
  %USERPROFILE%\.cache\ai-launcher so node_modules/target never enter the synced folder.

  .\local-build.ps1              UI + release exe (frontend embedded)
  .\local-build.ps1 -Test        also run Rust and UI checks
  .\local-build.ps1 -Icons       regenerate app/icons from assets/icon.png (copied back)
  .\local-build.ps1 -Bundle      installer (NSIS) via the Tauri CLI, copied to .\release
  .\local-build.ps1 -Run         start the built exe afterwards
#>
param([switch]$Test, [switch]$Icons, [switch]$Bundle, [switch]$Run)
# Native tools write progress to stderr; Windows PowerShell would turn that into terminating
# errors under 'Stop'. Every step checks $LASTEXITCODE instead.
$ErrorActionPreference = 'Continue'
$src = $PSScriptRoot
$cache = Join-Path $env:USERPROFILE '.cache\ai-launcher'
$work = Join-Path $cache 'src'
$env:CARGO_TARGET_DIR = Join-Path $cache 'target'
New-Item -ItemType Directory -Force $work | Out-Null

$localOnly = @('.git', 'node_modules', 'dist', 'target', 'gen', 'release') + @(Get-ChildItem -LiteralPath $src -Directory -Force -Filter '.*' | Where-Object Name -ne '.github' | ForEach-Object Name)
robocopy $src $work /MIR /XD $localOnly /XF *.log /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "mirror failed ($LASTEXITCODE)" }

$ui = Join-Path $work 'ui'
Push-Location $ui
try {
  & npm.cmd install --no-audit --no-fund
  if ($LASTEXITCODE -ne 0) { throw 'npm install failed' }
  Copy-Item (Join-Path $ui 'package-lock.json') (Join-Path $src 'ui\package-lock.json') -Force
  if ($Test) {
    & npm.cmd run check
    if ($LASTEXITCODE -ne 0) { throw 'svelte-check failed' }
  }
  & npm.cmd run build
  if ($LASTEXITCODE -ne 0) { throw 'ui build failed' }
} finally { Pop-Location }

$tauri = Join-Path $ui 'node_modules\.bin\tauri.cmd'
Push-Location $work
try {
  if ($Icons) {
    & $tauri icon (Join-Path $work 'assets\icon.png') -o (Join-Path $work 'app\icons')
    if ($LASTEXITCODE -ne 0) { throw 'icon generation failed' }
    foreach ($mobile in 'android', 'ios') { Remove-Item -Recurse -Force (Join-Path $work "app\icons\$mobile") -ErrorAction SilentlyContinue }
    Copy-Item (Join-Path $work 'app\icons\128x128@2x.png') (Join-Path $src 'ui\public\icon.png') -Force
    robocopy (Join-Path $work 'app\icons') (Join-Path $src 'app\icons') /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
  }
  if ($Test) {
    & cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw 'cargo test failed' }
  }
  Copy-Item (Join-Path $work 'Cargo.lock') (Join-Path $src 'Cargo.lock') -Force -ErrorAction SilentlyContinue
  if ($Bundle) {
    # Updater artifacts are signed with the key kept outside the repository (never committed).
    $key = Join-Path $env:USERPROFILE '.tauri\ai-launcher.key'
    $extra = @()
    if (Test-Path $key) {
      $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $key -Raw
      $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "$key.password" -Raw).Trim()
      $extra = @('--config', 'tauri.release.conf.json')
    } else { Write-Warning 'signing key not found: building an installer without update signatures' }
    Push-Location (Join-Path $work 'app')
    try {
      & $tauri build --bundles nsis @extra
      if ($LASTEXITCODE -ne 0) { throw 'bundle failed' }
    } finally {
      Pop-Location
      Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
    }
    $out = Join-Path $src 'release'
    New-Item -ItemType Directory -Force $out | Out-Null
    Get-ChildItem (Join-Path $env:CARGO_TARGET_DIR 'release\bundle\nsis') -File | Where-Object { $_.Extension -in '.exe', '.sig' } | Copy-Item -Destination $out -Force
    Write-Host "installer -> $out"
  } else {
    & cargo build --release -p ai-launcher --features custom-protocol
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
  }
  Copy-Item (Join-Path $work 'Cargo.lock') (Join-Path $src 'Cargo.lock') -Force -ErrorAction SilentlyContinue
} finally { Pop-Location }

$exe = Join-Path $env:CARGO_TARGET_DIR 'release\ai-launcher.exe'
Write-Host "exe: $exe"
if ($Run) { Start-Process $exe }
