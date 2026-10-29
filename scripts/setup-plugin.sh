#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP="$ROOT/_temp"
ARCHIVE="$TEMP/tauri-plugin-proxmark3-0.2.1.crate"
EXTRACT="$TEMP/plugin-crate"
PLUGIN="$EXTRACT/tauri-plugin-proxmark3-0.2.1"
URL="https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v0.2.1/tauri-plugin-proxmark3-0.2.1.crate"
SHA256="2361acc214fd0fd43fbf71fdeb9566807d44ea4730da47f3e9a03898093c0f1c"

mkdir -p "$TEMP" "$EXTRACT"

repair_links() {
  if grep -q 'links = "pm3bridge"' "$PLUGIN/Cargo.toml"; then
    sed -i.bak 's/links = "pm3bridge"/links = "proxmark3"/' "$PLUGIN/Cargo.toml"
    rm -f "$PLUGIN/Cargo.toml.bak"
    echo "Patched Cargo links key: pm3bridge -> proxmark3"
  fi
}

repair_android_cmake() {
  local lists="$PLUGIN/proxmark3/client/experimental_lib/CMakeLists.txt"
  [ -f "$lists" ] || return 0
  if grep -q 'mkdir -p ${BZIP2_BUILD_DIR}\|mkdir -p ${LZ4_BUILD_DIR}' "$lists"; then
    sed -i.bak \
      -e 's|mkdir -p ${BZIP2_BUILD_DIR}|${CMAKE_COMMAND} -E make_directory ${BZIP2_BUILD_DIR}|' \
      -e 's|mkdir -p ${LZ4_BUILD_DIR}|${CMAKE_COMMAND} -E make_directory ${LZ4_BUILD_DIR}|' \
      "$lists"
    rm -f "$lists.bak"
    echo "Patched Android ExternalProject mkdir commands"
  fi
}

# GUI builds on Windows have no console, so fd 1 is not a valid handle.  The
# upstream bridge assumed it could always dup() stdout, which made every
# terminal command fail with "cannot redirect stdout".
repair_stdout_capture() {
  local shim="$PLUGIN/shim/pm3_bridge.c"
  local patch="$ROOT/scripts/patches/pm3_bridge-stdout-capture.patch"
  [ -f "$shim" ] || return 0
  grep -q 'pm3x_save_stdout' "$shim" && return 0
  [ -f "$patch" ] || { echo "Missing patch: $patch" >&2; exit 1; }
  if ! git -C "$ROOT" apply --check "$patch"; then
    echo "Cannot apply $patch. The plugin sources may have changed." >&2
    exit 1
  fi
  git -C "$ROOT" apply "$patch"
  echo "Patched stdout capture for GUI builds"
}

# Keep the Windows release self-contained: MSYS2 bzip2/lz4/winpthread are
# available as static archives, and the app must start without MSYS2 on PATH.
repair_static_windows() {
  local build="$PLUGIN/build.rs"
  local patch="$ROOT/scripts/patches/pm3_build-static-windows.patch"
  [ -f "$build" ] || return 0
  grep -q 'static=pthread' "$build" && return 0
  [ -f "$patch" ] || { echo "Missing patch: $patch" >&2; exit 1; }
  if ! git -C "$ROOT" apply --check "$patch"; then
    echo "Cannot apply $patch. The plugin sources may have changed." >&2
    exit 1
  fi
  git -C "$ROOT" apply "$patch"
  echo "Patched Windows static library linking"
}

# Apple clang x86_64 does not provide GCC's __cpu_indicator_init/__cpu_model
# runtime, so the client's __builtin_cpu_supports() dispatchers fail to link.
# Use the portable NOSIMD paths there; arm64 still uses NEON.
repair_apple_simd() {
  local header="$PLUGIN/proxmark3/client/deps/hardnested/hardnested_bf_core.h"
  local patch="$ROOT/scripts/patches/pm3_apple-simd.patch"
  [ -f "$header" ] || return 0
  grep -q '&& !defined(__APPLE__)' "$header" && return 0
  [ -f "$patch" ] || { echo "Missing patch: $patch" >&2; exit 1; }
  if ! git -C "$ROOT" apply --check "$patch"; then
    echo "Cannot apply $patch. The plugin sources may have changed." >&2
    exit 1
  fi
  git -C "$ROOT" apply "$patch"
  echo "Patched Apple x86_64 SIMD dispatch"
}

if [ -f "$PLUGIN/Cargo.toml" ]; then
  repair_links
  repair_android_cmake
  repair_stdout_capture
  repair_static_windows
  repair_apple_simd
  echo "Proxmark3 plugin already present: $PLUGIN"
  exit 0
fi

echo "Downloading tauri-plugin-proxmark3 0.2.1..."
curl -fL "$URL" -o "$ARCHIVE"
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
else
  ACTUAL="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"
fi
if [ "$ACTUAL" != "$SHA256" ]; then
  rm -f "$ARCHIVE"
  echo "Plugin archive checksum mismatch: expected=$SHA256 actual=$ACTUAL" >&2
  exit 1
fi

rm -rf "$PLUGIN"
tar -xzf "$ARCHIVE" -C "$EXTRACT"
repair_links
repair_android_cmake
repair_stdout_capture
repair_static_windows
repair_apple_simd
echo "Proxmark3 plugin ready: $PLUGIN"
