use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri::State;

use crate::state::AppState;

#[derive(Serialize, Clone)]
struct BridgeData {
    data: String,
}

#[derive(Serialize, Clone)]
struct BridgeState {
    connected: bool,
}

pub struct BridgeHandle {
    pub port: u16,
    stop: Arc<AtomicBool>,
    writer: Sender<Vec<u8>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for BridgeHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn bridge_loop(
    app: AppHandle,
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    writer: Receiver<Vec<u8>>,
) {
    let _ = listener.set_nonblocking(true);
    let mut client: Option<TcpStream> = None;

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _address)) => {
                if let Ok(stream) = prepare_stream(stream) {
                    client = Some(stream);
                    let _ = app.emit("bridge://state", BridgeState { connected: true });
                    break;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(15));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let Some(mut stream) = client else {
        return;
    };
    let mut read_buffer = [0u8; 4096];

    while !stop.load(Ordering::SeqCst) {
        let mut wrote = false;
        while let Ok(data) = writer.try_recv() {
            if stream.write_all(&data).is_err() {
                break;
            }
            wrote = true;
        }
        if wrote {
            let _ = stream.flush();
        }

        match stream.read(&mut read_buffer) {
            Ok(0) => break,
            Ok(length) => {
                let payload = BridgeData {
                    data: STANDARD.encode(&read_buffer[..length]),
                };
                let _ = app.emit("bridge://data", payload);
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }

    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = app.emit("bridge://state", BridgeState { connected: false });
}

fn prepare_stream(stream: TcpStream) -> std::io::Result<TcpStream> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    pub port: u16,
}

#[tauri::command]
pub fn ble_bridge_start(app: AppHandle, state: State<'_, AppState>) -> Result<BridgeInfo, String> {
    let mut slot = state
        .bridge
        .lock()
        .map_err(|_| "BLE bridge state is unavailable.".to_string())?;
    if let Some(existing) = slot.as_ref() {
        return Ok(BridgeInfo {
            port: existing.port,
        });
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Could not open a loopback bridge: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the loopback bridge address: {error}"))?
        .port();
    let stop = Arc::new(AtomicBool::new(false));
    let (writer, receiver) = mpsc::channel::<Vec<u8>>();
    let app_handle = app.clone();
    let thread_stop = stop.clone();
    let thread = thread::spawn(move || bridge_loop(app_handle, listener, thread_stop, receiver));

    *slot = Some(BridgeHandle {
        port,
        stop,
        writer,
        thread: Some(thread),
    });
    Ok(BridgeInfo { port })
}

#[tauri::command]
pub fn ble_bridge_write(state: State<'_, AppState>, data: String) -> Result<(), String> {
    let decoded = STANDARD
        .decode(data.as_bytes())
        .map_err(|error| format!("Invalid bridge payload: {error}"))?;
    let slot = state
        .bridge
        .lock()
        .map_err(|_| "BLE bridge state is unavailable.".to_string())?;
    let bridge = slot
        .as_ref()
        .ok_or_else(|| "The BLE bridge is not running.".to_string())?;
    bridge
        .writer
        .send(decoded)
        .map_err(|_| "The BLE bridge has stopped.".to_string())
}

#[tauri::command]
pub fn ble_bridge_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut slot = state
        .bridge
        .lock()
        .map_err(|_| "BLE bridge state is unavailable.".to_string())?;
    if let Some(bridge) = slot.take() {
        drop(bridge);
    }
    Ok(())
}
