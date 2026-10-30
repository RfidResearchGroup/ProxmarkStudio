param(
  [switch]$Debug,
  [switch]$Release,
  [switch]$Apk,
  [string[]]$Targets = @('aarch64', 'armv7', 'i686', 'x86_64')
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
  $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
}

$sdkCandidates = @(
  $env:ANDROID_HOME,
  $env:ANDROID_SDK_ROOT,
  (Join-Path $env:LOCALAPPDATA 'Android\Sdk'),
  (Join-Path $env:USERPROFILE 'AppData\Local\Android\Sdk')
) | Where-Object { $_ -and (Test-Path (Join-Path $_ 'platforms')) }

$sdk = $sdkCandidates | Select-Object -First 1
if (-not $sdk) {
  throw 'Android SDK was not found. Set ANDROID_HOME to your SDK directory.'
}
$env:ANDROID_HOME = $sdk
$env:ANDROID_SDK_ROOT = $sdk

$ndk = $env:NDK_HOME
if (-not $ndk) {
  # Prefer a temporary NDK checked out next to the project, then the SDK's
  # newest installed NDK. Rust's Android target needs NDK r27+ (libunwind).
  $ndk = Get-ChildItem (Join-Path $root '_temp/android-ndk') -Directory -Filter 'android-ndk-*' -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndk) {
  $ndk = Get-ChildItem (Join-Path $sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndk) { throw 'Android NDK was not found. Install NDK r27+ and set NDK_HOME.' }
$env:NDK_HOME = $ndk
$env:ANDROID_NDK_HOME = $ndk

$javaCandidates = @(
  $env:JAVA_HOME,
  'C:\Program Files\Android\Android Studio\jbr',
  'C:\Program Files\Java\jdk-17',
  'C:\Program Files\Eclipse Adoptium\jdk-17',
  'C:\Program Files\Microsoft\jdk-17'
) | Where-Object { $_ -and (Test-Path (Join-Path $_ 'bin\java.exe')) }

if (-not $env:JAVA_HOME) {
  $discovered = Get-ChildItem 'C:\Program Files\Java','C:\Program Files\Eclipse Adoptium','C:\Program Files\Microsoft' -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path (Join-Path $_.FullName 'bin\java.exe') } |
    Select-Object -First 1 -ExpandProperty FullName
  if ($discovered) { $javaCandidates += $discovered }
}

$java = $javaCandidates | Select-Object -First 1
if ($java) { $env:JAVA_HOME = $java }
elseif (-not (Get-Command java -ErrorAction SilentlyContinue)) {
  throw 'A JDK 17+ was not found. Set JAVA_HOME.'
}

$msys = @(
  $env:PM3X_MINGW_BIN,
  'C:\msys64\ucrt64\bin',
  'C:\msys2\ucrt64\bin',
  'C:\tools\msys64\ucrt64\bin',
  'C:\UserProgram\msys2\ucrt64\bin'
) | Where-Object { $_ -and (Test-Path (Join-Path $_ 'gcc.exe')) } | Select-Object -First 1

$pathParts = @($cargoBin)
if ($msys) {
  $pathParts += $msys
  $msysRoot = Split-Path -Parent (Split-Path -Parent $msys)
  $pathParts += (Join-Path $msysRoot 'usr\bin')
}
$pathParts += $env:PATH
$env:PATH = $pathParts -join ';'
if ($msys) { $env:PM3X_MINGW_BIN = $msys }

$cli = Join-Path $root 'node_modules/@tauri-apps/cli/tauri.js'
if (-not (Test-Path $cli)) { throw 'Tauri CLI is missing. Run `pnpm install` first.' }

foreach ($target in $Targets) {
  Write-Host "Building Android target: $target"
  $cliArgs = @('android', 'build', '--target', $target, '--ci', '--ignore-version-mismatches')
  if (-not $Release) { $cliArgs += '--debug' }
  if ($Apk -or -not $Release) { $cliArgs += '--apk' }
  & node $cli @cliArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Tauri packages only the last target into its ABI-specific APK. All Rust
# libraries are now present in jniLibs, so repackage the universal APK once
# with every ABI included.
$profile = if ($Release) { 'Release' } else { 'Debug' }
Write-Host "Packaging universal APK with: $($Targets -join ', ')"
& (Join-Path $root 'src-tauri/gen/android/gradlew.bat') `
  --project-dir (Join-Path $root 'src-tauri/gen/android') `
  ":app:packageUniversal$profile" `
  '-x' "rustBuildUniversal$profile" `
  --no-daemon
exit $LASTEXITCODE
