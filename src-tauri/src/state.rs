use std::path::PathBuf;
use std::sync::Mutex;

use crate::bridge::BridgeHandle;

pub struct AppState {
    pub home: PathBuf,
    pub pm3_dir: PathBuf,
    pub bridge: Mutex<Option<BridgeHandle>>,
}

impl AppState {
    pub fn new(home: PathBuf) -> Self {
        let pm3_dir = home.join(".proxmark3");
        Self {
            home,
            pm3_dir,
            bridge: Mutex::new(None),
        }
    }
}
