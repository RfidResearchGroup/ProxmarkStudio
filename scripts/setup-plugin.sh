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
    sed -i 's/links = "pm3bridge"/links = "proxmark3"/' "$PLUGIN/Cargo.toml"
    echo "Patched Cargo links key: pm3bridge -> proxmark3"
  fi
}

repair_android_cmake() {
  local lists="$PLUGIN/proxmark3/client/experimental_lib/CMakeLists.txt"
  [ -f "$lists" ] || return 0
  if grep -q 'mkdir -p ${BZIP2_BUILD_DIR}\|mkdir -p ${LZ4_BUILD_DIR}' "$lists"; then
    sed -i \
      -e 's|mkdir -p ${BZIP2_BUILD_DIR}|${CMAKE_COMMAND} -E make_directory ${BZIP2_BUILD_DIR}|' \
      -e 's|mkdir -p ${LZ4_BUILD_DIR}|${CMAKE_COMMAND} -E make_directory ${LZ4_BUILD_DIR}|' \
      "$lists"
    echo "Patched Android ExternalProject mkdir commands"
  fi
}

if [ -f "$PLUGIN/Cargo.toml" ]; then
  repair_links
  repair_android_cmake
  echo "Proxmark3 plugin already present: $PLUGIN"
  exit 0
fi

echo "Downloading tauri-plugin-proxmark3 0.2.1..."
curl -fL "$URL" -o "$ARCHIVE"
ACTUAL="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
if [ "$ACTUAL" != "$SHA256" ]; then
  rm -f "$ARCHIVE"
  echo "Plugin archive checksum mismatch: expected=$SHA256 actual=$ACTUAL" >&2
  exit 1
fi

rm -rf "$PLUGIN"
tar -xzf "$ARCHIVE" -C "$EXTRACT"
repair_links
repair_android_cmake
echo "Proxmark3 plugin ready: $PLUGIN"
