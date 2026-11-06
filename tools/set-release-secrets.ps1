<#
  Registers the updater signing key as GitHub Actions secrets of ngnearSINE/ai-launcher.
  The key pair lives outside the repository in %USERPROFILE%\.tauri\ (back it up: without it,
  installed copies can no longer verify updates). Run once, or again after rotating the key.
#>
$ErrorActionPreference = 'Stop'
$gh = 'C:\Program Files\GitHub CLI\gh.exe'
$key = Join-Path $env:USERPROFILE '.tauri\ai-launcher.key'
if (-not (Test-Path $key)) { throw "signing key not found: $key" }
Get-Content $key -Raw | & $gh secret set TAURI_SIGNING_PRIVATE_KEY -R ngnearSINE/ai-launcher
(Get-Content "$key.password" -Raw).Trim() | & $gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD -R ngnearSINE/ai-launcher
& $gh secret list -R ngnearSINE/ai-launcher
