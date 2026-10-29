const COMMANDS: &[&str] = &[
    "list_usb",
    "open_usb",
    "list_spp",
    "open_spp",
    "scan_spp",
    "write",
    "close",
    "register_listener",
    "remove_listener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .try_build()
        .expect("failed to build the transport plugin");
}
