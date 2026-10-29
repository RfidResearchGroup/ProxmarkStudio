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
    [System.IO.File]::WriteAllText($Path, $content, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host 'Patched Cargo links key: pm3bridge -> proxmark3' -ForegroundColor Yellow
  }
}

# The upstream Android CMake uses `mkdir -p` inside ExternalProject commands.
# Windows CMake executes those through cmd.exe, where `mkdir -p` is invalid.
# `cmake -E make_directory` is portable and keeps the build working on all hosts.
function Repair-AndroidCMake {
  $cmakeLists = Join-Path $pluginDir 'proxmark3/client/experimental_lib/CMakeLists.txt'
  if (-not (Test-Path $cmakeLists)) { return }
  $content = Get-Content -Raw $cmakeLists
  $updated = $content.Replace(
    'mkdir -p ${BZIP2_BUILD_DIR}',
    '${CMAKE_COMMAND} -E make_directory ${BZIP2_BUILD_DIR}'
  ).Replace(
    'mkdir -p ${LZ4_BUILD_DIR}',
    '${CMAKE_COMMAND} -E make_directory ${LZ4_BUILD_DIR}'
  )
  if ($updated -ne $content) {
    [System.IO.File]::WriteAllText($cmakeLists, $updated, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host 'Patched Android ExternalProject mkdir commands' -ForegroundColor Yellow
  }
}

# GUI builds on Windows have no console, so fd 1 is not a valid handle.  The
# upstream bridge assumed it could always _dup() stdout, which made every
# terminal command fail with "cannot redirect stdout".  Apply the portable
# capture fix (see scripts/patches/pm3_bridge-stdout-capture.patch).
function Repair-StdoutCapture {
  $shim = Join-Path $pluginDir 'shim/pm3_bridge.c'
  if (-not (Test-Path $shim)) { return }
  if ((Get-Content -Raw $shim) -match 'pm3x_save_stdout') { return }

  $patch = Join-Path $PSScriptRoot 'patches/pm3_bridge-stdout-capture.patch'
  if (-not (Test-Path $patch)) { throw "Missing patch: $patch" }

  git -C $root apply --check $patch
  if ($LASTEXITCODE -ne 0) {
    throw "Cannot apply $patch. The plugin sources may have changed."
  }
  git -C $root apply $patch
  Write-Host 'Patched stdout capture for GUI builds' -ForegroundColor Yellow
}

# Keep the Windows release self-contained: MSYS2 bzip2/lz4/winpthread are
# available as static archives, and the app must start without MSYS2 on PATH.
function Repair-StaticWindows {
  $build = Join-Path $pluginDir 'build.rs'
  if (-not (Test-Path $build)) { return }
  if ((Get-Content -Raw $build) -match 'static=pthread') { return }

  $patch = Join-Path $PSScriptRoot 'patches/pm3_build-static-windows.patch'
  if (-not (Test-Path $patch)) { throw "Missing patch: $patch" }

  git -C $root apply --check $patch
  if ($LASTEXITCODE -ne 0) {
    throw "Cannot apply $patch. The plugin sources may have changed."
  }
  git -C $root apply $patch
  Write-Host 'Patched Windows static library linking' -ForegroundColor Yellow
}

# Apple clang x86_64 does not provide GCC's __cpu_indicator_init/__cpu_model
# runtime, so the client's __builtin_cpu_supports() dispatchers fail to link.
# Use the portable NOSIMD paths there; arm64 still uses NEON.
function Repair-AppleSIMD {
  $header = Join-Path $pluginDir 'proxmark3/client/deps/hardnested/hardnested_bf_core.h'
  if (-not (Test-Path $header)) { return }
  if ((Get-Content -Raw $header).Contains('&& !defined(__APPLE__)')) { return }

  $patch = Join-Path $PSScriptRoot 'patches/pm3_apple-simd.patch'
  if (-not (Test-Path $patch)) { throw "Missing patch: $patch" }

  git -C $root apply --check $patch
  if ($LASTEXITCODE -ne 0) {
    throw "Cannot apply $patch. The plugin sources may have changed."
  }
  git -C $root apply $patch
  Write-Host 'Patched Apple x86_64 SIMD dispatch' -ForegroundColor Yellow
}

if ((Test-Path $manifest) -and -not $Force) {
  Repair-Links -Path $manifest
  Repair-AndroidCMake
  Repair-StdoutCapture
  Repair-StaticWindows
  Repair-AppleSIMD
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
Repair-Links -Path $manifest
Repair-AndroidCMake
Repair-StdoutCapture
Repair-StaticWindows
Repair-AppleSIMD
Write-Host "Proxmark3 plugin ready: $pluginDir" -ForegroundColor Green
