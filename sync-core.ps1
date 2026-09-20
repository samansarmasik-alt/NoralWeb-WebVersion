#Requires -Version 5.1
<#
.SYNOPSIS
  Sync desktop core to web (one way: desktop -> web).
.DESCRIPTION
  fetch.rs, research.rs, neural.rs, nim.rs are copied byte-identical.
  Two repos, one project: core has a single source (desktop).
  Web-only code (src/main.rs, ui/, Dockerfile) is never touched.
.PARAMETER Desktop
  Desktop project path.
.EXAMPLE
  .\sync-core.ps1
#>
param(
  [string]$Desktop = "C:\Users\User\OneDrive\Masaüstü\nöral web"
)
$ErrorActionPreference = "Stop"
$webSrc = Join-Path $PSScriptRoot "src"
$deskSrc = Join-Path $Desktop "src"
foreach ($f in @("fetch.rs", "research.rs", "neural.rs", "nim.rs")) {
  $src = Join-Path $deskSrc $f
  if (-not (Test-Path -LiteralPath $src)) { throw "missing: $src" }
  Copy-Item -LiteralPath $src -Destination (Join-Path $webSrc $f) -Force
  Write-Output "copied: $f"
}
Write-Output "done - next: cargo check"
