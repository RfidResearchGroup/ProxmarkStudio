const COMMANDS: &[&str] = &[
    "list_usb",
    "open_usb",
    "list_spp",
    "open_spp",
    "write",
    "close",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .try_build()
        .expect("failed to build the transport plugin");
}
