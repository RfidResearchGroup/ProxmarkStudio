# Development

This document covers building, packaging and maintaining Proxmark Studio.
For the project overview and feature list, see [README.md](../README.md).

## Repository layout

```text
src/                  Vue app (views, components, stores, services, i18n)
src-tauri/            Tauri/Rust backend and desktop configuration
  src/system.rs       platform capabilities, paths, serial discovery
  src/files.rs        sandboxed workspace file operations
  src/http_server.rs  embedded HTTP file manager server
  src/resources.rs    GitHub release matching + resource installer
  src/bridge.rs       loopback TCP bridge for BLE/USB/SPP frames
plugins/tauri-plugin-transport  Android USB CDC-ACM + Bluetooth SPP bridge
scripts/              frontend runner, plugin bootstrap, platform helpers
docs/screenshots/     UI captures
src-tauri/gen/android Android Studio project (committed for manifest tweaks)
_temp/                local toolchain/plugin cache, ignored by git
```

## Prerequisites

- Node.js 20+ and pnpm 9+
- Rust stable
- CMake 3.19+, Ninja, a C/C++ toolchain
- Windows: MSYS2 UCRT64 (`gcc`, `cmake`, `ninja`, `zlib`, `bzip2`, `lz4`)
- Windows x86: MSYS2 MINGW32 target packages in addition to UCRT64 host tools
- Linux: GTK/WebKitGTK development packages required by Tauri
- Android: JDK 17, Android SDK, NDK r27+
- iOS: macOS with Xcode; no Apple signing identity is required for the
  unsigned CI artifact

The embedded client is GPLv3 and builds with the GNU ABI on Windows. MSVC is
not supported and fails with an explicit message from the plugin.

## Bootstrap

```bash
pnpm install
# Download the pinned plugin crate into _temp. The crate carries the upstream
# compatibility patches and applies them to the Proxmark3 client at build time.
powershell -ExecutionPolicy Bypass -File scripts/setup-plugin.ps1   # Windows
# or
bash scripts/setup-plugin.sh                                        # Linux/macOS
```

The plugin is not committed. `scripts/setup-plugin.*` pins the release URL and
SHA-256, so the source tree stays small and reproducible. The current pin is
`tauri-plugin-proxmark3` 0.2.6.

## Windows

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup target add x86_64-pc-windows-gnu

# optional if MSYS2 is not at the default path
$env:PM3X_MINGW_BIN = 'C:\msys64\ucrt64\bin'

powershell -ExecutionPolicy Bypass -File scripts/tauri-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts/tauri-windows.ps1 -Release
powershell -ExecutionPolicy Bypass -File scripts/tauri-windows.ps1 -Release -Bundle
```

Direct CLI:

```powershell
node node_modules/@tauri-apps/cli/tauri.js build --target x86_64-pc-windows-gnu --no-bundle
```

### Windows x86

```powershell
rustup target add i686-pc-windows-gnu

# MINGW32 is the target toolchain; UCRT64 remains first on PATH for host tools.
$env:PATH = 'C:\msys64\ucrt64\bin;C:\msys64\mingw32\bin;C:\msys64\usr\bin;' + $env:PATH
$env:PM3X_MINGW_BIN = 'C:\msys64\mingw32\bin'

pnpm tauri build --target i686-pc-windows-gnu --bundles nsis
```

`.cargo/config.toml` uses `gcc` for the x64 host and explicitly selects
`i686-w64-mingw32-gcc` for the x86 target. The plugin's MQTT compatibility patch
avoids redefining `ssize_t` on MinGW.

The Windows release links bzip2, lz4, zlib and winpthread statically, so the
produced `.exe` runs without MSYS2 installed.

## Linux / macOS

```bash
pnpm install
bash scripts/setup-plugin.sh
pnpm tauri dev          # or: pnpm tauri build
```

On Linux, pair the device in the system Bluetooth settings and bind an rfcomm
port (`/dev/rfcomm*`). The app lists it alongside USB CDC ports; the embedded
client is built with Bluetooth transport support disabled, so SPP is handled
through the OS serial layer.

## Android

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android

# generated project is committed, so no `android init` is required
pnpm tauri android dev
# or, on Windows, use the environment helper:
powershell -ExecutionPolicy Bypass -File scripts/tauri-android.ps1 -Debug -Apk
```

`scripts/tauri-android.ps1` builds `aarch64`, `armv7`, `i686` and `x86_64`, then
repackages the universal APK so all four `libproxmark_studio_lib.so` variants are
included. If the NDK is under `_temp/android-ndk/android-ndk-r27c`, the script
finds it automatically; otherwise set `NDK_HOME` to an r27+ installation. The
script also discovers the Android SDK and a JDK 17+ from the standard locations.

Android x86/x86_64 link the NDK compiler-rt builtins so clang's
`__builtin_cpu_supports`/`__builtin_cpu_init` CPU dispatch keeps working.

Android USB serial is implemented by the local `tauri-plugin-transport` crate.
It opens CDC-ACM bulk endpoints, bridges them to the loopback TCP transport, and
exposes paired-classic Bluetooth devices over RFCOMM SPP the same way. The
plugin requests USB access on first use; Android shows its standard permission
dialog.

The generated Android project lives in
[`src-tauri/gen/android`](../src-tauri/gen/android) and is committed because the
manifest carries the share `FileProvider` and the resource paths used by the
app. It uses `compileSdk = 36` and `minSdk = 26`. Android BLE runtime permissions
are provided by `tauri-plugin-blec`; sharing uses the checked-in
`${applicationId}.fileprovider` provider.

## iOS

The iOS project is generated on macOS:

```bash
pnpm tauri ios init
```

Build an **unsigned** device `.app` for external signing:

```bash
CODE_SIGNING_ALLOWED=NO \
CODE_SIGNING_REQUIRED=NO \
pnpm tauri ios build --debug --target aarch64 --no-sign
```

Package the resulting bundle:

```bash
APP="$(find src-tauri/gen/apple/build -type d -name '*.app' -print -quit)"
ditto -c -k --sequesterRsrc --keepParent "$APP" ProxmarkStudio-ios-arm64.app.zip
```

CI produces `ProxmarkStudio-<version>-ios-arm64.app.zip`. The archive is not
signed; signing and provisioning are handled by a separate automation
environment. `src-tauri/Info.plist` includes the Bluetooth usage descriptions.
iOS deliberately has no USB or SPP support.

## Automated builds

[`.github/workflows/build.yml`](../.github/workflows/build.yml) builds and
packages:

| Platform | Runner | Output |
| --- | --- | --- |
| Windows x64 | `windows-latest` | NSIS installer + portable ZIP |
| Windows x86 | `windows-latest` | NSIS installer + portable ZIP |
| macOS Intel | `macos-15-intel` | `.dmg` |
| macOS Apple Silicon | `macos-14` | `.dmg` |
| Linux x64 | `ubuntu-22.04` | AppImage + Debian package |
| Android universal | `ubuntu-latest` | APK containing arm64-v8a, armeabi-v7a, x86 and x86_64 |
| Android arm64 | `ubuntu-latest` | smaller arm64-v8a-only APK |
| iOS arm64 | `macos-15` | unsigned `.app` ZIP |

The workflow runs on every push to `main`, pull requests, and manual dispatch.
Successful `main` builds update the rolling **Latest development build**
prerelease using debug binaries, while pushing a `v*` tag publishes a versioned
GitHub release built with optimized release profiles. Builds are also available
as workflow artifacts for 14 days.

### Versioning

[`scripts/version.mjs`](../scripts/version.mjs) is the single source of truth:

| Build | Version |
| --- | --- |
| `v*` tag | the tag without `v`, e.g. `v0.2.1` → `0.2.1` |
| `main` / Latest development | the newest reachable `v*` tag; `0.0.0` if none exist |
| Local developer build | `0.0.0-local` |

CI injects the computed version into the Tauri config at build time. Local
builds use the fixed `0.0.0-local` value committed in
[`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json). Set
`PROXMARK_STUDIO_VERSION` to override the local default deliberately.

### Creating a release

1. Make sure the release content is merged and the working tree is clean.
2. Create and push an annotated tag whose name is the release version:

   ```bash
   git tag -a v0.1.0 -m "Proxmark Studio 0.1.0"
   git push origin v0.1.0
   ```

3. The `v*` tag run builds **release** artifacts:
   - desktop `tauri build` release binaries
   - Android release libraries and release APKs
   - iOS release `.app` without signing
   - canonical release asset names, using the tag version
4. Verify the GitHub Actions run and the published release assets.

The tag version is the application version; there is no separate version field
to keep in sync. Latest development builds use the newest tag as their base
version so they remain comparable with the next release.

Android release APKs are currently signed with the standard Android debug key
so CI artifacts remain installable without repository secrets. Replace this
with a production keystore before distributing outside the project; the
optimized release build itself is unchanged.

Asset names follow this format:

```text
ProxmarkStudio-<version>-<platform>-<arch>-<kind>
```

For example:

```text
ProxmarkStudio-0.1.0-windows-x64-setup.exe
ProxmarkStudio-0.1.0-windows-x86-portable.zip
ProxmarkStudio-0.1.0-linux-x64.AppImage
ProxmarkStudio-0.1.0-macos-arm64.dmg
ProxmarkStudio-0.1.0-android-universal.apk
ProxmarkStudio-0.1.0-android-arm64.apk
ProxmarkStudio-0.1.0-ios-arm64.app.zip
```

## Runtime data

| Data | Location |
| --- | --- |
| Client HOME / workspace | `<app-data>/.proxmark3` |
| Installed resources | `<app-data>/.proxmark3/{cmdscripts,dictionaries,lualibs,luascripts,pyscripts,resources}` |
| Resource marker | `<app-data>/.proxmark3/.pm3-resources.json` |
| Download cache | `<app-data>/cache` |
| App preferences | Local storage (`pm3-studio.*` keys) |

`<app-data>` is `%APPDATA%\cn.forestweave.proxmarkstudio` on Windows,
`~/Library/Application Support/cn.forestweave.proxmarkstudio` on macOS,
and the XDG data directory on Linux.

## Notes and limitations

- The Proxmark3 client prints terminal-oriented output. The backend normalizes
  ANSI sequences for structured views; the terminal intentionally keeps the
  original stream for xterm.js.
- Only one Proxmark client session can be active per process; commands are
  serialized by the plugin.
- Windows only supports the GNU ABI for the embedded client.
- No telemetry is collected. The only network traffic is the explicit GitHub
  release lookup/download and the opt-in HTTP file server.
- The Proxmark3 client is GPLv3. This application statically links it, so
  redistributed binaries must comply with GPLv3. iOS App Store distribution has
  known GPLv3 compatibility concerns; evaluate before shipping there.

## License

GPL-3.0-or-later. See [LICENSE](../LICENSE). Proxmark3 and the Proxmark3 client
are copyright their respective contributors; `tauri-plugin-proxmark3` is by
[xianglin1998](https://github.com/xianglin1998).
