use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;
use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::files::{safe_existing_path, safe_path, FileEntry};
use crate::state::AppState;

const HTML: &str = include_str!("http_files.html");

pub struct HttpHandle {
    stop: Arc<AtomicBool>,
    port: u16,
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpInfo {
    pub url: String,
    pub port: u16,
    pub token: String,
    pub addresses: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub name: String,
    pub address: String,
}

fn network_interfaces() -> Vec<NetworkInterface> {
    let mut interfaces = Vec::new();
    if let Ok(items) = get_if_addrs::get_if_addrs() {
        for item in items {
            if let std::net::IpAddr::V4(address) = item.addr.ip() {
                if address.is_loopback() {
                    continue;
                }
                interfaces.push(NetworkInterface {
                    name: item.name,
                    address: address.to_string(),
                });
            }
        }
    }
    interfaces.sort_by(|a, b| a.address.cmp(&b.address));
    interfaces
}

fn local_addresses() -> Vec<String> {
    let mut addresses = network_interfaces()
        .into_iter()
        .map(|interface| interface.address)
        .collect::<Vec<_>>();
    addresses.push("127.0.0.1".to_string());
    addresses
}

#[tauri::command]
pub async fn list_network_interfaces() -> Result<Vec<NetworkInterface>, String> {
    Ok(network_interfaces())
}

fn query_value(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key {
            return urlencoding::decode(value).ok().map(|value| value.into_owned());
        }
    }
    None
}

fn list_entries(root: &Path, path: &str) -> Result<Vec<FileEntry>, String> {
    let directory = safe_existing_path(root, path)?;
    if !directory.is_dir() {
        return Err("Not a directory.".to_string());
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Could not resolve workspace: {error}"))?;
    let mut entries = Vec::new();
    for item in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let item = item.map_err(|error| error.to_string())?;
        let metadata = item.metadata().map_err(|error| error.to_string())?;
        let absolute = item.path();
        let relative = absolute
            .strip_prefix(&canonical_root)
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        entries.push(FileEntry {
            name: item.file_name().to_string_lossy().to_string(),
            path: relative,
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() { metadata.len() } else { 0 },
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
            extension: absolute
                .extension()
                .map(|value| value.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap()
}

fn html_response() -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_string(HTML);
    response.add_header(
        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
    );
    response
}

fn json_response(value: serde_json::Value, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_string(value.to_string()).with_status_code(StatusCode(status));
    response.add_header(json_header());
    response
}

fn error_response(message: &str, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(serde_json::json!({ "error": message }), status)
}

fn parse_body<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T, String> {
    let mut body = Vec::new();
    request.as_reader().read_to_end(&mut body).map_err(|error| error.to_string())?;
    serde_json::from_slice(&body).map_err(|error| error.to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn parse_multipart(body: &[u8], content_type: &str) -> Vec<(String, Vec<u8>)> {
    let Some(boundary) = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("boundary="))
        .map(|value| value.trim_matches('"').to_string())
    else {
        return Vec::new();
    };
    let delimiter = format!("--{boundary}").into_bytes();
    let mut files = Vec::new();
    let mut offset = 0;
    while let Some(start) = find_bytes(&body[offset..], &delimiter) {
        let part_start = offset + start + delimiter.len();
        if body.get(part_start..part_start + 2) == Some(b"--") {
            break;
        }
        let Some(header_end) = find_bytes(&body[part_start..], b"\r\n\r\n") else {
            break;
        };
        let headers = String::from_utf8_lossy(&body[part_start..part_start + header_end]);
        let data_start = part_start + header_end + 4;
        let Some(next) = find_bytes(&body[data_start..], &delimiter) else {
            break;
        };
        let data_end = data_start + next;
        if let Some(filename) = headers.split("filename=\"").nth(1).and_then(|value| value.split('"').next()) {
            let name = Path::new(filename)
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "upload.bin".to_string());
            let mut data = body[data_start..data_end].to_vec();
            while data.ends_with(b"\r\n") {
                data.truncate(data.len() - 2);
            }
            files.push((name, data));
        }
        offset = data_end;
    }
    files
}

fn handle(mut request: Request, root: PathBuf, token: &str) {
    let url = request.url().to_string();
    if query_value(&url, "token").as_deref() != Some(token) {
        let _ = request.respond(error_response("Invalid or missing access token.", 403));
        return;
    }
    let path = url.split('?').next().unwrap_or("/");
    let result: Response<std::io::Cursor<Vec<u8>>> = match (request.method().as_str(), path) {
        ("GET", "/") | ("GET", "/index.html") => html_response(),
        ("GET", "/api/list") => {
            let target = query_value(&url, "path").unwrap_or_default();
            match list_entries(&root, &target) {
                Ok(entries) => json_response(serde_json::json!({ "path": target, "entries": entries }), 200),
                Err(error) => error_response(&error, 400),
            }
        }
        ("GET", "/api/download") => {
            let target = query_value(&url, "path").unwrap_or_default();
            match safe_existing_path(&root, &target) {
                Ok(file) if file.is_file() => match fs::read(&file) {
                    Ok(data) => {
                        let mut response = Response::from_data(data);
                        let name = file.file_name().unwrap_or_default().to_string_lossy();
                        response.add_header(
                            Header::from_bytes(
                                &b"Content-Disposition"[..],
                                format!("attachment; filename=\"{}\"", name.replace('"', "")).as_bytes(),
                            )
                            .unwrap(),
                        );
                        response.add_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..]).unwrap(),
                        );
                        response
                    }
                    Err(error) => error_response(&error.to_string(), 500),
                },
                Ok(_) => error_response("Not a file.", 400),
                Err(error) => error_response(&error, 404),
            }
        }
        ("POST", "/api/upload") => {
            let target = query_value(&url, "path").unwrap_or_default();
            let overwrite = query_value(&url, "overwrite").as_deref() == Some("1");
            let content_type = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Content-Type"))
                .map(|header| header.value.as_str().to_string())
                .unwrap_or_default();
            let mut body = Vec::new();
            if let Err(error) = request.as_reader().read_to_end(&mut body) {
                let _ = request.respond(error_response(&error.to_string(), 400));
                return;
            }
            let files = parse_multipart(&body, &content_type);
            if files.is_empty() {
                error_response("No files were uploaded.", 400)
            } else {
                match safe_existing_path(&root, &target) {
                    Ok(directory) if directory.is_dir() => {
                        for (name, data) in &files {
                            let relative = if target.is_empty() {
                                name.clone()
                            } else {
                                format!("{}/{}", target.trim_matches('/'), name)
                            };
                            if let Ok(destination) = safe_path(&root, &relative) {
                                if destination.exists() && !overwrite {
                                    let _ = request.respond(error_response("A file already exists.", 409));
                                    return;
                                }
                                let _ = fs::write(destination, data);
                            }
                        }
                        json_response(serde_json::json!({ "ok": true }), 200)
                    }
                    _ => error_response("Invalid destination.", 400),
                }
            }
        }
        ("POST", "/api/delete") => match parse_body::<serde_json::Value>(&mut request) {
            Ok(body) => {
                let target = body.get("path").and_then(|value| value.as_str()).unwrap_or_default();
                match safe_existing_path(&root, target) {
                    Ok(path) if path != root.canonicalize().unwrap_or(root.clone()) => {
                        let result = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
                        match result {
                            Ok(()) => json_response(serde_json::json!({ "ok": true }), 200),
                            Err(error) => error_response(&error.to_string(), 500),
                        }
                    }
                    _ => error_response("Cannot delete this path.", 400),
                }
            }
            Err(error) => error_response(&error, 400),
        },
        ("POST", "/api/mkdir") => match parse_body::<serde_json::Value>(&mut request) {
            Ok(body) => {
                let parent = body.get("parent").and_then(|value| value.as_str()).unwrap_or_default();
                let name = body.get("name").and_then(|value| value.as_str()).unwrap_or_default();
                if name.is_empty() || name.contains(['/', '\\']) {
                    error_response("Invalid folder name.", 400)
                } else {
                    let relative = if parent.is_empty() { name.to_string() } else { format!("{}/{}", parent.trim_matches('/'), name) };
                    match safe_path(&root, &relative) {
                        Ok(path) if !path.exists() => match fs::create_dir(path) {
                            Ok(()) => json_response(serde_json::json!({ "ok": true }), 200),
                            Err(error) => error_response(&error.to_string(), 500),
                        },
                        _ => error_response("The folder already exists.", 409),
                    }
                }
            }
            Err(error) => error_response(&error, 400),
        },
        ("POST", "/api/move") => match parse_body::<serde_json::Value>(&mut request) {
            Ok(body) => {
                let source = body.get("source").and_then(|value| value.as_str()).unwrap_or_default();
                let target = body.get("target").and_then(|value| value.as_str()).unwrap_or_default();
                match safe_existing_path(&root, source) {
                    Ok(from) => {
                        let name = body
                            .get("name")
                            .and_then(|value| value.as_str())
                            .filter(|value| !value.is_empty())
                            .unwrap_or_else(|| from.file_name().unwrap_or_default().to_str().unwrap_or(""));
                        let relative = if target.is_empty() { name.to_string() } else { format!("{}/{}", target.trim_matches('/'), name) };
                        match safe_path(&root, &relative) {
                            Ok(to) if !to.exists() => match fs::rename(from, to) {
                                Ok(()) => json_response(serde_json::json!({ "ok": true }), 200),
                                Err(error) => error_response(&error.to_string(), 500),
                            },
                            _ => error_response("The destination already exists.", 409),
                        }
                    }
                    Err(error) => error_response(&error, 404),
                }
            }
            Err(error) => error_response(&error, 400),
        },
        _ => error_response("Not found.", 404),
    };
    let _ = request.respond(result);
}

#[tauri::command]
pub async fn start_http_server(
    state: State<'_, AppState>,
    bind_address: Option<String>,
    password: String,
) -> Result<HttpInfo, String> {
    if password.len() < 4 || password.len() > 12 || !password.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err("The password must be 4-12 letters or digits.".to_string());
    }
    {
        let mut current = state.http.lock().map_err(|error| error.to_string())?;
        if let Some(handle) = current.take() {
            handle.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.1", handle.port));
        }
    }
    let bind_host = bind_address
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "0.0.0.0".to_string());
    let server = match Server::http(format!("{}:8787", bind_host)) {
        Ok(server) => server,
        Err(_) => Server::http(format!("{}:0", bind_host)).map_err(|error| error.to_string())?,
    };
    let port = server.server_addr().to_ip().map(|addr| addr.port()).unwrap_or(8787);
    let token = password;
    let stop = Arc::new(AtomicBool::new(false));
    let root = state.pm3_dir.clone();
    let thread_stop = stop.clone();
    let thread_token = token.clone();
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match server.recv_timeout(Duration::from_millis(250)) {
                Ok(Some(request)) => handle(request, root.clone(), &thread_token),
                Ok(None) => {}
                Err(_) => break,
            }
        }
    });
    let mut addresses = local_addresses();
    if !addresses.contains(&bind_host) && bind_host != "0.0.0.0" {
        addresses.insert(0, bind_host.clone());
    }
    let host = if bind_host == "0.0.0.0" {
        addresses
            .iter()
            .find(|address| !address.starts_with("127."))
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    } else {
        bind_host
    };
    let url = format!("http://{}:{}/?token={}", host, port, token);
    let info = HttpInfo {
        url,
        port,
        token: token.clone(),
        addresses,
    };
    *state.http.lock().map_err(|error| error.to_string())? = Some(HttpHandle { stop, port, token });
    Ok(info)
}

#[tauri::command]
pub async fn stop_http_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut current = state.http.lock().map_err(|error| error.to_string())?;
    if let Some(handle) = current.take() {
        handle.stop.store(true, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(("127.0.0.1", handle.port));
        let _ = handle.token;
    }
    Ok(())
}

#[tauri::command]
pub async fn http_server_status(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let current = state.http.lock().map_err(|error| error.to_string())?;
    Ok(current.as_ref().map(|handle| format!("http://127.0.0.1:{}/", handle.port)))
}
