param(
  [switch]$Debug,
  [switch]$Release,
  [switch]$Apk
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Prefer a local _temp Rust install if present; otherwise rely on the system.
$cargoBin = ''
if (Test-Path (Join-Path $root '_temp/cargo/bin')) {
  $cargoBin = Join-Path $root '_temp/cargo/bin'
  $env:CARGO_HOME = Join-Path $root '_temp/cargo'
  $env:RUSTUP_HOME = Join-Path $root '_temp/rustup'
}

$env:ANDROID_HOME = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { 'C:\Users\dxl\AppData\Local\Android\Sdk' }
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$ndk = $env:NDK_HOME
if (-not $ndk) {
  # Prefer a temporary NDK checked out next to the project, then the SDK's
  # newest installed NDK. Rust's Android target needs NDK r27+ (libunwind).
  $localNdk = Get-ChildItem (Join-Path $root '_temp/android-ndk') -Directory -Filter 'android-ndk-*' -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
  $ndk = $localNdk
}
if (-not $ndk) {
  $ndk = Get-ChildItem (Join-Path $env:ANDROID_HOME 'ndk') -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndk) { throw 'Android NDK was not found. Install NDK r27+ and set NDK_HOME.' }
$env:NDK_HOME = $ndk
$env:ANDROID_NDK_HOME = $ndk
$env:JAVA_HOME = if ($env:JAVA_HOME) { $env:JAVA_HOME } else { 'C:\ProgramData\openjdk1701' }
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
$env:PATH = @(
  $cargoBin,
  'C:\UserProgram\msys2\ucrt64\bin',
  'C:\UserProgram\msys2\usr\bin',
  $env:PATH
) -join ';'

$cli = Join-Path $root 'node_modules/@tauri-apps/cli/tauri.js'
if (-not (Test-Path $cli)) { throw 'Tauri CLI is missing. Run `pnpm install` first.' }

$cliArgs = @('android', 'build', '--target', 'aarch64', '--ci', '--ignore-version-mismatches')
if (-not $Release) { $cliArgs += '--debug' }
if ($Apk -or -not $Release) { $cliArgs += '--apk' }

& node $cli @cliArgs
exit $LASTEXITCODE
