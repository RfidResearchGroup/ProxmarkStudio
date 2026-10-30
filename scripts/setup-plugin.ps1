param(
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$temp = Join-Path $root '_temp'
$version = '0.2.4'
$archive = Join-Path $temp "tauri-plugin-proxmark3-$version.crate"
$extractRoot = Join-Path $temp 'plugin-crate'
$pluginDir = Join-Path $extractRoot "tauri-plugin-proxmark3-$version"
$manifest = Join-Path $pluginDir 'Cargo.toml'
$url = "https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v$version/tauri-plugin-proxmark3-$version.crate"
$sha256 = 'f09b813a56f965619b98e4d10ac97cf81dd38f450c226b0f85073733ee8595c0'

New-Item -ItemType Directory -Force $temp, $extractRoot | Out-Null

if ((Test-Path $manifest) -and -not $Force) {
  Write-Host "Proxmark3 plugin already present: $pluginDir" -ForegroundColor Green
  exit 0
}

Write-Host "Downloading tauri-plugin-proxmark3 $version..." -ForegroundColor Cyan
if (Test-Path $archive) { Remove-Item $archive -Force }
Invoke-WebRequest -UseBasicParsing $url -OutFile $archive

$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $sha256) {
  Remove-Item $archive -Force
  throw "Plugin archive checksum mismatch. expected=$sha256 actual=$actual"
}

if (Test-Path $pluginDir) { Remove-Item $pluginDir -Recurse -Force }
# Use the Windows bsdtar explicitly. MSYS2's GNU tar (which setup-msys2 puts on
# PATH) treats the drive letter in D:\... as a remote host.
$tar = Join-Path $env:SystemRoot 'System32/tar.exe'
if (-not (Test-Path $tar)) { $tar = 'tar' }
& $tar -xzf $archive -C $extractRoot
if ($LASTEXITCODE -ne 0) {
  throw "Could not extract the plugin archive with $tar"
}
if (-not (Test-Path $manifest)) {
  throw "Extraction failed; expected $manifest"
}

Write-Host "Proxmark3 plugin ready: $pluginDir" -ForegroundColor Green
