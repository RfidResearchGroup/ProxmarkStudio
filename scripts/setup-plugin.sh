#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP="$ROOT/_temp"
VERSION="0.2.5"
ARCHIVE="$TEMP/tauri-plugin-proxmark3-$VERSION.crate"
EXTRACT="$TEMP/plugin-crate"
PLUGIN="$EXTRACT/tauri-plugin-proxmark3-$VERSION"
URL="https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v$VERSION/tauri-plugin-proxmark3-$VERSION.crate"
SHA256="3a8aef7c565e6ae2128e46023c77c88bfc6742b342f0665e03f44f94e6753383"

mkdir -p "$TEMP" "$EXTRACT"

if [ -f "$PLUGIN/Cargo.toml" ]; then
  echo "Proxmark3 plugin already present: $PLUGIN"
  exit 0
fi

echo "Downloading tauri-plugin-proxmark3 $VERSION..."
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
if [ ! -f "$PLUGIN/Cargo.toml" ]; then
  echo "Extraction failed; expected $PLUGIN/Cargo.toml" >&2
  exit 1
fi
echo "Proxmark3 plugin ready: $PLUGIN"
