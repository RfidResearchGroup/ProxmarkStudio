fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PM3_CLIENT_COMMIT");
    let commit = std::env::var("PM3_CLIENT_COMMIT")
        .unwrap_or_else(|_| "d596fe6b04fb90356f04433eb82ff1ae354ada40".to_string());
    println!("cargo:rustc-env=PM3_CLIENT_COMMIT={commit}");
    tauri_build::build()
}
