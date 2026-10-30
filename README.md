# Proxmark Studio

A cross-platform workbench for **Proxmark3 and Proxmark5** hardware, built with
[Tauri 2](https://v2.tauri.app/start/), Vue 3, Pinia and TypeScript.

Proxmark Studio wraps the embedded Proxmark client from
[`tauri-plugin-proxmark3`](https://github.com/xianglin1998/tauri-plugin-proxmark3)
in a focused desktop/mobile interface: connect over USB serial, Bluetooth LE or
Bluetooth SPP, inspect the device, browse its working directory and run client
commands in an xterm.js terminal.

The UI is model-neutral. Device names, targets and transports are read from the
connected client at runtime, so the same build works with RDV4, GENERIC,
ULTIMATE, ICOPVX, Proxmark5 and other Proxmark variants.

![Proxmark Studio home](docs/screenshots/home.png)

## Features

- **Connect** over USB serial, Bluetooth LE or Bluetooth SPP where the platform
  and client support it. Unsupported transports are shown as unavailable rather
  than failing silently.
- **Device overview** with live hardware/client identification (`hw version`,
  `hw status`), transport, port and session details.
- **Integrated terminal** powered by xterm.js, with command history, streaming
  output and the same command grammar as the `pm3` client.
- **Workspace file manager**: browse the client's `~/.proxmark3` directory,
  preview text or hex, move files, create folders, delete and share/reveal.
- **HTTP file manager**: start a local web server and manage the workspace from
  another device's browser, including uploads, downloads and drag-to-move.
- **Client resources** downloaded from the matching plugin GitHub release. The
  app resolves the release by the client commit, verifies SHA-256, and installs
  the archive into the client's user resource layout automatically.
- **Dark-first design** with four themes (Abyss blue by default), a desktop
  sidebar, a mobile tab bar, and a frameless desktop window with custom chrome.
- **English + Simplified Chinese**, following the system language by default and
  switchable in Settings.

| Desktop | Terminal | Files | Settings |
| --- | --- | --- | --- |
| ![Home](docs/screenshots/home.png) | ![Terminal](docs/screenshots/terminal.png) | ![Files](docs/screenshots/files.png) | ![Settings](docs/screenshots/settings.png) |

## Platform support

| Capability | Windows | macOS | Linux | Android | iOS |
| --- | --- | --- | --- | --- | --- |
| USB serial | Yes | Yes | Yes | Yes (CDC-ACM) | Unsupported by iOS |
| Bluetooth LE | Yes | Yes | Yes | Yes | Yes |
| Bluetooth SPP | Yes (paired COM) | Yes (paired serial) | Yes (paired rfcomm) | Yes (paired device) | Unsupported by iOS |
| Share sheet | Reveal in Explorer | Reveal in Finder | Open folder | Yes | Yes |
| HTTP file manager | Yes | Yes | Yes | Yes | Yes |
| Frameless desktop chrome | Yes | Yes | Yes | n/a | n/a |

The capability flags come from
[`src-tauri/src/system.rs`](src-tauri/src/system.rs). If a transport is not
available, its tab is disabled and the UI explains why; it is never offered as a
dead control.

> Android USB serial uses a native CDC-ACM bridge; Android SPP uses an RFCOMM
> socket. Both are exposed through the local loopback bridge, so the embedded
> client sees the same TCP transport it uses for BLE.

## HTTP file manager

Some platforms keep app files inside a sandbox that is awkward to reach from
another computer, and iOS does not expose a general user-visible filesystem for
the client workspace. The built-in HTTP file manager solves that without a
desktop sync tool:

1. Open **Files** and start the HTTP server.
2. Pick a network interface and a short alphanumeric access password.
3. Open the shown LAN URL (`http://<address>:<port>/?token=<password>`) on a
   phone, tablet or desktop browser.
4. Upload, download, rename, delete, create folders and drag files/folders
   between folders.

The page is embedded in the app, works at phone widths and follows the app's
English/Chinese language setting. The server runs only while explicitly started,
binds to the selected interface, requires the token on every request, rejects
uploads over 32 MB and stops when the app is closed. This is intentionally a
local-network convenience, not an internet-facing service.

## Bluetooth LE

The app bridges BLE UART frames to a loopback TCP socket consumed by the
embedded client. This keeps the client transport layer unchanged while allowing
BLE on every platform supported by `tauri-plugin-blec`.

Auto-detection understands:

| Device | Service | Characteristic | Notes |
| --- | --- | --- | --- |
| Proxmark5 BWM ESP32 | `0xAE86` | `0xAE88` | Read + write + notify on one characteristic, per the [BWM DEV.md](https://github.com/RfidResearchGroup/Proxmark5_BWM_esp32/blob/master/DEV.md) |
| Nordic UART bridge | `6E400001-B5A3-F393-E0A9-E50E24DCCA9E` | RX `6E400002-B5A3-F393-E0A9-E50E24DCCA9E`, TX `6E400003-B5A3-F393-E0A9-E50E24DCCA9E` | Common third-party BLE add-ons |
| Generic UART | any | first write + notify pair | HM-10 and similar modules |

The Proxmark5 BWM values are the defaults in Settings. Custom service and
characteristic UUIDs can be entered there if auto-detection is not enough.

## Client resources

1. The app reads the compiled client version and extracts its git hash when one
   is present, falling back to the pinned commit in
   [`src-tauri/build.rs`](src-tauri/build.rs).
2. It walks the plugin's GitHub releases from newest to oldest and reads each
   release's `manifest.json`.
3. The first release whose `proxmark3.commit` matches is selected. The matching
   `proxmark3-client-resources-<commit>.zip` is downloaded from that release.
4. The archive is checked against the published SHA-256, extracted to a staging
   directory, and the `share/proxmark3/*` payload is installed into
   `~/.proxmark3` (user resource layout) so the client finds dictionaries,
   scripts and Lua libraries without any extra environment setup.
5. A hidden `.pm3-resources.json` marker records commit, version, asset and file
   count.

![Installed client resources](docs/screenshots/resources.png)

The app's app-data directory is used as `HOME` for the embedded client on every
platform. On Windows the C runtime's home is updated through the plugin bridge
itself so the statically linked client sees the same directory.

## Stack

- **Shell**: Tauri 2.12 (Rust), `tauri-plugin-proxmark3` 0.2.6 (static libpm3)
- **UI**: Vue 3.5, Pinia, Vue Router, Vite 6, TypeScript
- **Terminal**: `@xterm/xterm` + `@xterm/addon-fit`
- **Transport**: `tauri-plugin-blec` for BLE, serialport for USB/SPP discovery,
  and the local `tauri-plugin-transport` for Android USB CDC-ACM + Bluetooth SPP
- **Resources**: reqwest (rustls) + zip + sha2
- **HTTP server**: tiny_http with the embedded file-manager page
- **Mobile sharing**: `tauri-plugin-share`

## Releases

Release assets use a stable name format:

```text
ProxmarkStudio-<version>-<platform>-<arch>-<kind>
```

Examples:

```text
ProxmarkStudio-0.1.0-windows-x64-setup.exe
ProxmarkStudio-0.1.0-windows-x86-portable.zip
ProxmarkStudio-0.1.0-linux-x64.AppImage
ProxmarkStudio-0.1.0-macos-arm64.dmg
ProxmarkStudio-0.1.0-android-universal.apk
ProxmarkStudio-0.1.0-android-arm64.apk
ProxmarkStudio-0.1.0-ios-arm64.app.zip
```

The iOS artifact is an unsigned `.app` bundle archive for external signing.
Pushes to `main` publish debug builds to the rolling latest prerelease; pushing
a `v*` tag publishes optimized release builds with the same canonical names.

## Development

Build instructions, platform prerequisites, the release-asset naming rules and
maintainer notes live in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Proxmark3 and the Proxmark3 client are
copyright their respective contributors; `tauri-plugin-proxmark3` is by
[xianglin1998](https://github.com/xianglin1998).
