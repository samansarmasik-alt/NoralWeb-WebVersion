#Requires -Version 5.1
<#
.SYNOPSIS
  Masaüstü çekirdeği web cephesine senkronlar (tek yön: desktop → web).
.DESCRIPTION
  fetch.rs, research.rs, neural.rs, nim.rs BİREBİR kopyalanır — iki depo
  aynı projenin iki yarısıdır, çekirdek tek kaynaktan (masaüstü) beslenir.
  Web'e özel her şey src/main.rs + ui/ + Dockerfile içindedir, kopya dokunmaz.
.PARAMETER Desktop
  Masaüstü projesinin yolu.
.EXAMPLE
  .\sync-core.ps1
  .\sync-core.ps1 -Desktop "D:\projeler\noral web"
#>
param(
  [string]$Desktop = "C:\Users\User\OneDrive\Masaüstü\nöral web"
)
$ErrorActionPreference = "Stop"
$webSrc = Join-Path $PSScriptRoot "src"
$deskSrc = Join-Path $Desktop "src"
foreach ($f in @("fetch.rs", "research.rs", "neural.rs", "nim.rs")) {
  $src = Join-Path $deskSrc $f
  if (-not (Test-Path -LiteralPath $src)) { throw "bulunamadı: $src" }
  Copy-Item -LiteralPath $src -Destination (Join-Path $webSrc $f) -Force
  Write-Output "kopyalandı: $f"
}
Write-Output "tamam — sonra: cargo check && cargo test"
