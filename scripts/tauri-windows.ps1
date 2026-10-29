param(
  [switch]$Release,
  [switch]$Bundle
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not (Test-Path 'node_modules')) {
  throw 'Frontend dependencies are missing. Run `pnpm install` first.'
}
if (-not (Test-Path '_temp/plugin-crate/tauri-plugin-proxmark3-0.2.2/Cargo.toml')) {
  & (Join-Path $PSScriptRoot 'setup-plugin.ps1')
}

$msysCandidates = @(
  $env:PM3X_MINGW_BIN,
  'C:\msys64\ucrt64\bin',
  'C:\msys2\ucrt64\bin',
  'C:\tools\msys64\ucrt64\bin',
  'C:\UserProgram\msys2\ucrt64\bin'
) | Where-Object { $_ -and (Test-Path (Join-Path $_ 'gcc.exe')) }

$msys = $msysCandidates | Select-Object -First 1
if (-not $msys) {
  throw 'MSYS2 UCRT64 gcc was not found. Install MSYS2 or set PM3X_MINGW_BIN to its ucrt64\bin directory.'
}

$cargoBin = ''
$localRust = Join-Path $root '_temp/cargo/bin'
if (Test-Path $localRust) {
  $cargoBin = $localRust
  $env:CARGO_HOME = Join-Path $root '_temp/cargo'
  $env:RUSTUP_HOME = Join-Path $root '_temp/rustup'
}

$msysRoot = Split-Path -Parent (Split-Path -Parent $msys)
$msysUsrBin = Join-Path $msysRoot 'usr\bin'
$env:PATH = @($cargoBin, $msys, $msysUsrBin, $env:PATH) -join ';'
$env:PM3X_MINGW_BIN = $msys
$cmake = Join-Path $msys 'cmake.exe'
if (Test-Path $cmake) { $env:CMAKE = $cmake }

$rustup = if ($cargoBin) { Join-Path $cargoBin 'rustup.exe' } else { (Get-Command rustup -ErrorAction SilentlyContinue).Source }
if ($rustup -and (Test-Path $rustup)) {
  $toolchains = & $rustup toolchain list 2>$null
  if (-not ($toolchains -match 'stable-x86_64-pc-windows-gnu')) {
    throw 'The GNU host toolchain is required for the Windows build. Run: rustup toolchain install stable-x86_64-pc-windows-gnu'
  }
}
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'

$cli = Join-Path $root 'node_modules/@tauri-apps/cli/tauri.js'
if (-not (Test-Path $cli)) {
  throw 'Tauri CLI is missing. Run `pnpm install` first.'
}

$mode = if ($Release) { 'build' } else { 'dev' }
$extra = @('--target', 'x86_64-pc-windows-gnu')
if ($Release -and -not $Bundle) { $extra += '--no-bundle' }

& node $cli $mode @extra
exit $LASTEXITCODE
