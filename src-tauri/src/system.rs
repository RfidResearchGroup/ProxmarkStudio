use serde::Serialize;
use tauri::AppHandle;

use crate::files::safe_existing_path;
use crate::state::AppState;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub usb_serial: bool,
    pub ble: bool,
    pub spp: bool,
    pub share: bool,
    pub native_files: bool,
    pub desktop: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub version: String,
    pub tauri_version: String,
    pub capabilities: Capabilities,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    pub work_dir: String,
    pub pm3_dir: String,
    pub resources_dir: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortInfo {
    pub path: String,
    pub kind: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

pub fn platform_capabilities() -> Capabilities {
    Capabilities {
        // The embedded client on Android/iOS has no tty layer; USB serial is a
        // desktop capability only.
        usb_serial: !is_mobile(),
        ble: true,
        // SPP requires an operating-system serial binding. Android would need a
        // dedicated native RFCOMM bridge; iOS does not expose SPP at all.
        spp: cfg!(not(any(target_os = "android", target_os = "ios"))),
        share: is_mobile(),
        native_files: true,
        desktop: !is_mobile(),
    }
}

#[tauri::command]
pub fn get_platform_info(app: AppHandle) -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: if is_mobile() { "mobile" } else { "desktop" }.to_string(),
        version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION.to_string(),
        capabilities: platform_capabilities(),
    }
}

#[tauri::command]
pub fn get_paths(state: tauri::State<'_, AppState>) -> PathInfo {
    PathInfo {
        work_dir: state.pm3_dir.display().to_string(),
        pm3_dir: state.pm3_dir.display().to_string(),
        resources_dir: state.home.join("share").join("proxmark3").display().to_string(),
    }
}

#[tauri::command]
pub fn get_diagnostics(app: AppHandle, state: tauri::State<'_, AppState>) -> String {
    let caps = platform_capabilities();
    let client_version = tauri_plugin_proxmark3::client::version();
    format!(
        "Proxmark Studio {}\nOS: {} ({})\nClient: {}\nPinned commit: {}\nHome: {}\nWorkspace: {}\nCapabilities: USB serial={}, BLE={}, SPP={}, Share={}",
        app.package_info().version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        if client_version.is_empty() { "unknown" } else { &client_version },
        option_env!("PM3_CLIENT_COMMIT").unwrap_or("unknown"),
        state.home.display(),
        state.pm3_dir.display(),
        caps.usb_serial,
        caps.ble,
        caps.spp,
        caps.share,
    )
}

fn spawn_reveal(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if path.is_dir() {
            std::process::Command::new("explorer.exe")
                .arg(path)
                .spawn()
                .map_err(|error| error.to_string())?;
        } else {
            std::process::Command::new("explorer.exe")
                .arg(format!("/select,{}", path.display()))
                .spawn()
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if path.is_file() {
            command.arg("-R");
        }
        command.arg(path).spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))]
    {
        // xdg-open cannot select a file; open its parent instead.
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Revealing files is not supported on this platform.".to_string())
}

#[tauri::command]
pub fn reveal_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let target = safe_existing_path(&state.pm3_dir, &path)?;
    spawn_reveal(&target)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    tauri::async_runtime::spawn_blocking(list_serial_ports_blocking)
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn list_serial_ports_blocking() -> Result<Vec<SerialPortInfo>, String> {
    use serialport::SerialPortType;

    let ports = serialport::available_ports().map_err(|error| error.to_string())?;
    let mut result = Vec::with_capacity(ports.len());
    for port in ports {
        let (kind, manufacturer, product, serial_number, vid, pid) = match port.port_type {
            SerialPortType::UsbPort(info) => (
                "usb",
                info.manufacturer,
                info.product,
                info.serial_number,
                Some(info.vid),
                Some(info.pid),
            ),
            SerialPortType::BluetoothPort => ("bluetooth", None, None, None, None, None),
            SerialPortType::PciPort => ("unknown", None, None, None, None, None),
            SerialPortType::Unknown => ("unknown", None, None, None, None, None),
        };
        result.push(SerialPortInfo {
            path: port.port_name,
            kind: kind.to_string(),
            manufacturer,
            product,
            serial_number,
            vid,
            pid,
        });
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    Ok(Vec::new())
}


