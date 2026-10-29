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
if (-not (Test-Path '_temp/plugin-crate/tauri-plugin-proxmark3-0.2.1/Cargo.toml')) {
  & (Join-Path $PSScriptRoot 'setup-plugin.ps1')
}

$msys = $env:PM3X_MINGW_BIN
if (-not $msys) { $msys = 'C:\UserProgram\msys2\ucrt64\bin' }
if (-not (Test-Path (Join-Path $msys 'gcc.exe'))) {
  throw "MSYS2 UCRT64 gcc was not found at $msys. Set PM3X_MINGW_BIN."
}

$cargoBin = ''
$localRust = Join-Path $root '_temp/cargo/bin'
if (Test-Path $localRust) {
  $cargoBin = $localRust
  $env:CARGO_HOME = Join-Path $root '_temp/cargo'
  $env:RUSTUP_HOME = Join-Path $root '_temp/rustup'
}

$env:PATH = @($cargoBin, $msys, 'C:\UserProgram\msys2\usr\bin', $env:PATH) -join ';'
$env:PM3X_MINGW_BIN = $msys
$env:CMAKE = Join-Path $msys 'cmake.exe'
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
