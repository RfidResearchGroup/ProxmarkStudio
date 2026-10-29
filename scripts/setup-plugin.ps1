param(
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$temp = Join-Path $root '_temp'
$archive = Join-Path $temp 'tauri-plugin-proxmark3-0.2.1.crate'
$extractRoot = Join-Path $temp 'plugin-crate'
$pluginDir = Join-Path $extractRoot 'tauri-plugin-proxmark3-0.2.1'
$manifest = Join-Path $pluginDir 'Cargo.toml'
$url = 'https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v0.2.1/tauri-plugin-proxmark3-0.2.1.crate'
$sha256 = '2361acc214fd0fd43fbf71fdeb9566807d44ea4730da47f3e9a03898093c0f1c'

New-Item -ItemType Directory -Force $temp, $extractRoot | Out-Null

function Repair-Links {
  param([string]$Path)
  $content = Get-Content -Raw $Path
  if ($content -match 'links = "pm3bridge"') {
    $content = $content.Replace('links = "pm3bridge"', 'links = "proxmark3"')
    Set-Content -NoNewline -Encoding utf8 $Path $content
    Write-Host 'Patched Cargo links key: pm3bridge -> proxmark3' -ForegroundColor Yellow
  }
}

if ((Test-Path $manifest) -and -not $Force) {
  Repair-Links -Path $manifest
  Write-Host "Proxmark3 plugin already present: $pluginDir" -ForegroundColor Green
  exit 0
}

Write-Host "Downloading tauri-plugin-proxmark3 0.2.1..." -ForegroundColor Cyan
if (Test-Path $archive) { Remove-Item $archive -Force }
Invoke-WebRequest -UseBasicParsing $url -OutFile $archive

$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $sha256) {
  Remove-Item $archive -Force
  throw "Plugin archive checksum mismatch. expected=$sha256 actual=$actual"
}

if (Test-Path $pluginDir) { Remove-Item $pluginDir -Recurse -Force }
tar -xzf $archive -C $extractRoot
if (-not (Test-Path $manifest)) {
  throw "Extraction failed; expected $manifest"
}
Repair-Links -Path $manifest
Write-Host "Proxmark3 plugin ready: $pluginDir" -ForegroundColor Green
