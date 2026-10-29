mod bridge;
mod files;
mod resources;
mod state;
mod system;

use std::ffi::CString;
use std::fs;
use std::os::raw::c_char;
use std::path::Path;

use tauri::Manager;

use state::AppState;

extern "C" {
    /// C entry point exported by the Proxmark plugin's bridge. Calling it here
    /// updates the C runtime environment as well as the process environment,
    /// which is required on Windows where Rust's `set_var` is not visible to
    /// the statically linked client's `getenv`.
    fn pm3x_set_home(dir: *const c_char);
}

fn set_client_home(home: &Path) {
    if let Ok(value) = CString::new(home.to_string_lossy().as_bytes()) {
        unsafe { pm3x_set_home(value.as_ptr()) };
    }
}

fn resolve_home<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let home = app
        .path()
        .app_data_dir()
        .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
    fs::create_dir_all(home.join(".proxmark3"))?;
    Ok(home)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_share::init())
        .plugin(tauri_plugin_blec::init())
        .plugin(tauri_plugin_proxmark3::init())
        .setup(|app| {
            let home = resolve_home(app.handle())?;
            fs::create_dir_all(home.join("cache"))?;
            set_client_home(&home);
            app.manage(AppState::new(home));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system::get_platform_info,
            system::get_paths,
            system::get_diagnostics,
            system::reveal_path,
            system::list_serial_ports,
            files::list_dir,
            files::read_file,
            files::delete_path,
            files::move_path,
            files::create_folder,
            resources::check_resources,
            resources::get_release_info,
            resources::install_resources,
            resources::resources_folder,
            bridge::ble_bridge_start,
            bridge::ble_bridge_write,
            bridge::ble_bridge_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Proxmark Studio");
}
