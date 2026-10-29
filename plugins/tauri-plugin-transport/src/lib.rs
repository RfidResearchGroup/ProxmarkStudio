//! Android-only transport bridge (USB CDC-ACM and Bluetooth SPP).
//!
//! The Kotlin side owns the OS resources; this Rust shim registers the plugin,
//! loads the Android `TransportPlugin` class and exposes the ACL namespace.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("transport")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _api.register_android_plugin("app.tauri.transport", "TransportPlugin")?;
            Ok(())
        })
        .build()
}
