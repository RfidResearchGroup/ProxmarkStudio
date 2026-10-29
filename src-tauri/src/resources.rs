use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};
use zip::ZipArchive;

use crate::files::{safe_existing_path, write_workspace_file};
use crate::state::AppState;

const API_RELEASES: &str =
    "https://api.github.com/repos/xianglin1998/tauri-plugin-proxmark3/releases?per_page=20";
const USER_AGENT_VALUE: &str = "Proxmark-Studio/0.1 (+https://github.com/xianglin1998/tauri-plugin-proxmark3)";
const PINNED_COMMIT: &str = env!("PM3_CLIENT_COMMIT");
const MARKER_FILE: &str = ".pm3-resources.json";
const RESOURCE_DIRS: [&str; 6] = [
    "cmdscripts",
    "dictionaries",
    "lualibs",
    "luascripts",
    "pyscripts",
    "resources",
];
const MAX_ARCHIVE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 250 * 1024 * 1024;
const MAX_ENTRIES: usize = 20000;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatus {
    pub installed: bool,
    pub valid: bool,
    pub path: String,
    pub expected_commit: String,
    pub commit: Option<String>,
    pub version: Option<String>,
    pub installed_at: Option<u64>,
    pub file_count: usize,
    pub expected_files: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag: String,
    pub name: String,
    pub published_at: String,
    pub html_url: String,
    pub asset_name: String,
    pub asset_size: u64,
    pub checksum: Option<String>,
    pub download_url: String,
    pub manifest_url: String,
    pub client_version: String,
    pub commit: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub status: ResourceStatus,
    pub release: ReleaseInfo,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceProgress {
    pub stage: String,
    pub downloaded: u64,
    pub total: u64,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Marker {
    commit: String,
    version: String,
    installed_at: u64,
    file_count: usize,
    release_tag: String,
    asset: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct Manifest {
    proxmark3: ManifestPm3,
    client_resources: ManifestResources,
    #[serde(default)]
    assets: Vec<ManifestAsset>,
}

#[derive(Deserialize)]
struct ManifestPm3 {
    commit: String,
    #[serde(default)]
    short_commit: String,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct ManifestResources {
    file: String,
}

#[derive(Deserialize)]
struct ManifestAsset {
    name: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

fn emit_progress(
    app: &AppHandle,
    stage: &str,
    downloaded: u64,
    total: u64,
    message: Option<String>,
) {
    let _ = app.emit(
        "resources://progress",
        ResourceProgress {
            stage: stage.to_string(),
            downloaded,
            total,
            message,
        },
    );
}

fn commit_from_version(version: &str) -> Option<String> {
    let index = version.rfind('g')?;
    let candidate: String = version[index + 1..]
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .collect();
    if candidate.len() >= 7 {
        Some(candidate)
    } else {
        None
    }
}

/// Return the full commit when the compiled client's version string identifies
/// the pinned revision, otherwise the hash reported by the client.
pub fn current_client_commit() -> String {
    let version = tauri_plugin_proxmark3::client::version();
    if let Some(short) = commit_from_version(&version) {
        if PINNED_COMMIT.starts_with(&short) {
            return PINNED_COMMIT.to_string();
        }
        return short;
    }
    PINNED_COMMIT.to_string()
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(12).collect()
}

fn marker_path(pm3_dir: &Path) -> PathBuf {
    pm3_dir.join(MARKER_FILE)
}

fn read_marker(pm3_dir: &Path) -> Option<Marker> {
    let data = fs::read_to_string(marker_path(pm3_dir)).ok()?;
    serde_json::from_str::<Marker>(&data).ok()
}

fn count_files(path: &Path) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut count = 0;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            count += count_files(&entry.path());
        } else if file_type.is_file() {
            count += 1;
        }
    }
    count
}

fn resource_status(pm3_dir: &Path, expected_commit: &str) -> ResourceStatus {
    let marker = read_marker(pm3_dir);
    let dirs_present = RESOURCE_DIRS
        .iter()
        .all(|name| pm3_dir.join(name).is_dir());
    let installed = dirs_present || marker.is_some();
    let commit_matches = marker
        .as_ref()
        .map(|value| {
            value.commit == expected_commit
                || value.commit.starts_with(expected_commit)
                || expected_commit.starts_with(&value.commit)
                || value.commit.starts_with(&short_commit(expected_commit))
        })
        .unwrap_or(false);
    let file_count = marker
        .as_ref()
        .map(|value| value.file_count)
        .filter(|count| *count > 0)
        .unwrap_or_else(|| {
            RESOURCE_DIRS
                .iter()
                .map(|name| count_files(&pm3_dir.join(name)))
                .sum()
        });

    ResourceStatus {
        installed,
        valid: installed && commit_matches,
        path: pm3_dir.display().to_string(),
        expected_commit: expected_commit.to_string(),
        commit: marker.as_ref().map(|value| value.commit.clone()),
        version: marker.as_ref().map(|value| value.version.clone()),
        installed_at: marker.as_ref().map(|value| value.installed_at),
        file_count,
        expected_files: RESOURCE_DIRS.len(),
    }
}

#[cfg(windows)]
fn windows_system_proxy() -> Option<String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut enabled = false;
    let mut server = None;
    for line in text.lines() {
        if line.contains("ProxyEnable") {
            enabled = line.trim_end().ends_with('1');
        } else if line.contains("ProxyServer") {
            server = line.split_whitespace().last().map(str::to_string);
        }
    }
    if !enabled {
        return None;
    }
    let raw = server?;
    let candidate = if let Some(value) = raw.split(';').find_map(|part| part.strip_prefix("https=")) {
        value
    } else if let Some(value) = raw.split(';').find_map(|part| part.strip_prefix("http=")) {
        value
    } else {
        raw.as_str()
    };
    if candidate.starts_with("http://") || candidate.starts_with("https://") || candidate.starts_with("socks5://") {
        Some(candidate.to_string())
    } else {
        Some(format!("http://{candidate}"))
    }
}

fn proxy_url() -> Option<String> {
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    #[cfg(windows)]
    {
        return windows_system_proxy();
    }
    #[allow(unreachable_code)]
    None
}

fn http_client() -> Result<Client, String> {
    let mut builder = Client::builder()
        .user_agent(USER_AGENT_VALUE)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(120));
    if let Some(proxy) = proxy_url() {
        builder = builder
            .proxy(reqwest::Proxy::all(&proxy).map_err(|error| format!("Invalid proxy configuration: {error}"))?);
    }
    builder
        .build()
        .map_err(|error| format!("Could not create the HTTP client: {error}"))
}

fn fetch_json<T: for<'de> Deserialize<'de>>(client: &Client, url: &str) -> Result<T, String> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|error| format!("Request to {url} failed: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        let detail: String = body.chars().take(300).collect();
        if status.as_u16() == 403 || status.as_u16() == 429 {
            return Err(format!(
                "GitHub API rate limit reached ({status}). Try again later or download the resource archive manually. {detail}"
            ));
        }
        return Err(format!("GitHub returned {status}: {detail}"));
    }
    response
        .json::<T>()
        .map_err(|error| format!("Could not parse the response from {url}: {error}"))
}

fn release_matches(manifest: &Manifest, expected_commit: &str) -> bool {
    let commit = &manifest.proxmark3.commit;
    let short = if manifest.proxmark3.short_commit.is_empty() {
        short_commit(commit)
    } else {
        manifest.proxmark3.short_commit.clone()
    };
    commit == expected_commit
        || commit.starts_with(expected_commit)
        || expected_commit.starts_with(commit)
        || short.starts_with(expected_commit)
        || expected_commit.starts_with(&short)
}

fn find_release(client: &Client, expected_commit: &str) -> Result<Option<ReleaseInfo>, String> {
    let releases: Vec<GitHubRelease> = fetch_json(client, API_RELEASES)?;
    for release in releases.into_iter().filter(|release| !release.draft) {
        let Some(manifest_asset) = release
            .assets
            .iter()
            .find(|asset| asset.name == "manifest.json")
        else {
            continue;
        };
        let manifest_url = manifest_asset.browser_download_url.clone();
        let manifest: Manifest = match fetch_json(client, &manifest_url) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };
        if !release_matches(&manifest, expected_commit) {
            continue;
        }
        let resource_name = manifest.client_resources.file.clone();
        let Some(resource_asset) = release.assets.iter().find(|asset| asset.name == resource_name)
        else {
            continue;
        };
        let download_url = resource_asset.browser_download_url.clone();
        let asset_name = resource_asset.name.clone();
        let manifest_asset_info = manifest
            .assets
            .iter()
            .find(|asset| asset.name == resource_name);
        let checksum = manifest_asset_info.and_then(|asset| asset.sha256.clone());
        let asset_size = if resource_asset.size > 0 {
            resource_asset.size
        } else {
            manifest_asset_info
                .and_then(|asset| asset.size)
                .unwrap_or(0)
        };
        return Ok(Some(ReleaseInfo {
            tag: release.tag_name,
            name: release
                .name
                .unwrap_or_else(|| "Proxmark3 client resources".to_string()),
            published_at: release.published_at.unwrap_or_default(),
            html_url: release.html_url,
            asset_name,
            asset_size,
            checksum,
            download_url,
            manifest_url,
            client_version: manifest.proxmark3.version,
            commit: manifest.proxmark3.commit,
        }));
    }
    Ok(None)
}

#[tauri::command]
pub async fn check_resources(state: State<'_, AppState>) -> Result<ResourceStatus, String> {
    let pm3_dir = state.pm3_dir.clone();
    let commit = current_client_commit();
    Ok(resource_status(&pm3_dir, &commit))
}

#[tauri::command]
pub async fn get_release_info() -> Result<Option<ReleaseInfo>, String> {
    let commit = current_client_commit();
    tauri::async_runtime::spawn_blocking(move || {
        let client = http_client()?;
        find_release(&client, &commit)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn remove_cache(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

fn extract_archive(
    app: &AppHandle,
    archive_path: &Path,
    staging: &Path,
) -> Result<usize, String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("Could not open the downloaded archive: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("The downloaded archive is invalid: {error}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("The resource archive contains too many entries.".to_string());
    }
    fs::create_dir_all(staging)
        .map_err(|error| format!("Could not create the staging folder: {error}"))?;

    let entry_count = archive.len();
    let mut extracted_bytes = 0u64;
    let mut file_count = 0usize;
    for index in 0..entry_count {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read archive entry: {error}"))?;
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                continue;
            }
        }
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let normalized = enclosed.to_string_lossy().replace('\\', "/");
        let Some(relative) = normalized.strip_prefix("share/proxmark3/") else {
            continue;
        };
        if relative.is_empty() || relative == MARKER_FILE {
            continue;
        }
        if entry.is_dir() {
            fs::create_dir_all(staging.join(relative))
                .map_err(|error| format!("Could not create an extracted folder: {error}"))?;
            continue;
        }
        if entry.size() > MAX_ARCHIVE_BYTES {
            return Err(format!("Archive entry {relative} is too large."));
        }
        extracted_bytes = extracted_bytes.saturating_add(entry.size());
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err("The resource archive expands beyond the safety limit.".to_string());
        }
        let destination = staging.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create an extracted folder: {error}"))?;
        }
        let mut output = File::create(&destination)
            .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Could not extract {relative}: {error}"))?;
        file_count += 1;
        if index % 64 == 0 {
            emit_progress(
                app,
                "extracting",
                index as u64,
                entry_count as u64,
                Some(relative.to_string()),
            );
        }
    }

    for directory in RESOURCE_DIRS {
        if !staging.join(directory).is_dir() {
            return Err(format!(
                "The resource archive is missing the required directory '{directory}'."
            ));
        }
    }
    emit_progress(
        app,
        "extracting",
        entry_count as u64,
        entry_count as u64,
        Some("Resources unpacked".to_string()),
    );
    Ok(file_count)
}

fn install_staging(pm3_dir: &Path, staging: &Path) -> Result<(), String> {
    let entries = fs::read_dir(staging)
        .map_err(|error| format!("Could not read the staging folder: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read the staging folder: {error}"))?;
    let mut installed: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();

    for entry in entries {
        let name = entry.file_name();
        if name.to_string_lossy() == MARKER_FILE {
            continue;
        }
        let source = entry.path();
        let destination = pm3_dir.join(&name);
        let backup = if destination.exists() {
            let backup = pm3_dir.join(format!(".pm3-backup-{}", name.to_string_lossy()));
            if backup.exists() {
                if backup.is_dir() {
                    fs::remove_dir_all(&backup)
                        .map_err(|error| format!("Could not clear backup {}: {error}", backup.display()))?;
                } else {
                    fs::remove_file(&backup)
                        .map_err(|error| format!("Could not clear backup {}: {error}", backup.display()))?;
                }
            }
            fs::rename(&destination, &backup).map_err(|error| {
                format!(
                    "Could not stage the existing {}: {error}",
                    destination.display()
                )
            })?;
            Some(backup)
        } else {
            None
        };

        match fs::rename(&source, &destination) {
            Ok(()) => installed.push((destination, backup)),
            Err(error) => {
                if let Some(backup) = backup {
                    let _ = fs::rename(&backup, &destination);
                }
                for (destination, backup) in installed.iter().rev() {
                    let _ = fs::remove_dir_all(destination);
                    if let Some(backup) = backup {
                        let _ = fs::rename(backup, destination);
                    }
                }
                return Err(format!(
                    "Could not install resources into {}: {error}",
                    destination.display()
                ));
            }
        }
    }

    for (_, backup) in installed {
        if let Some(backup) = backup {
            let _ = if backup.is_dir() {
                fs::remove_dir_all(&backup)
            } else {
                fs::remove_file(&backup)
            };
        }
    }
    Ok(())
}

fn download_and_install(
    app: &AppHandle,
    home: &Path,
    pm3_dir: &Path,
    expected_commit: &str,
) -> Result<InstallResult, String> {
    emit_progress(app, "fetching", 0, 0, None);
    let client = http_client()?;
    let release = find_release(&client, expected_commit)?
        .ok_or_else(|| "No plugin release matches the compiled Proxmark3 client commit.".to_string())?;

    let cache_dir = home.join("cache");
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Could not create the cache folder: {error}"))?;
    let archive_path = cache_dir.join(format!("resources-{}.zip", std::process::id()));
    let staging = cache_dir.join(format!("resources-{}-staging", std::process::id()));

    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("Could not clear the old staging folder: {error}"))?;
    }

    let response = client
        .get(&release.download_url)
        .send()
        .map_err(|error| format!("Could not download the resource archive: {error}"))?;
    let response = response
        .error_for_status()
        .map_err(|error| format!("The resource download failed: {error}"))?;
    let total = response.content_length().unwrap_or(release.asset_size);
    if total > MAX_ARCHIVE_BYTES {
        return Err("The resource archive exceeds the safety size limit.".to_string());
    }

    let mut output = File::create(&archive_path)
        .map_err(|error| format!("Could not create the download file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut last_emit = Instant::now();
    let mut reader = response;

    loop {
        let mut buffer = [0u8; 64 * 1024];
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Could not read the download stream: {error}"))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| format!("Could not write the downloaded archive: {error}"))?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        if downloaded > MAX_ARCHIVE_BYTES {
            remove_cache(&archive_path);
            return Err("The resource archive exceeds the safety size limit.".to_string());
        }
        if last_emit.elapsed() > Duration::from_millis(120) {
            emit_progress(app, "downloading", downloaded, total, None);
            last_emit = Instant::now();
        }
    }
    output
        .flush()
        .map_err(|error| format!("Could not finish the downloaded archive: {error}"))?;
    drop(output);
    emit_progress(app, "downloading", downloaded, downloaded, None);

    let digest = hex::encode(hasher.finalize());
    if let Some(checksum) = &release.checksum {
        if !checksum.eq_ignore_ascii_case(&digest) {
            remove_cache(&archive_path);
            return Err(format!(
                "The downloaded archive failed its SHA-256 check (expected {checksum}, got {digest})."
            ));
        }
    }
    emit_progress(app, "verifying", downloaded, downloaded, None);

    let extraction = extract_archive(app, &archive_path, &staging);
    remove_cache(&archive_path);
    let file_count = match extraction {
        Ok(count) => count,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    if let Err(error) = install_staging(pm3_dir, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let _ = fs::remove_dir_all(&staging);

    let installed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let marker = Marker {
        commit: release.commit.clone(),
        version: release.client_version.clone(),
        installed_at,
        file_count,
        release_tag: release.tag.clone(),
        asset: release.asset_name.clone(),
    };
    let marker_json = serde_json::to_vec_pretty(&marker)
        .map_err(|error| format!("Could not serialize the resource marker: {error}"))?;
    write_workspace_file(pm3_dir, MARKER_FILE, &marker_json)?;

    emit_progress(app, "done", downloaded, downloaded, None);
    Ok(InstallResult {
        status: resource_status(pm3_dir, expected_commit),
        release,
    })
}

#[tauri::command]
pub async fn install_resources(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InstallResult, String> {
    let home = state.home.clone();
    let pm3_dir = state.pm3_dir.clone();
    let expected_commit = current_client_commit();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        download_and_install(&app_handle, &home, &pm3_dir, &expected_commit)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Used by Settings to open the resource folder in the desktop file manager.
#[tauri::command]
pub fn resources_folder(state: State<'_, AppState>) -> Result<String, String> {
    let path = safe_existing_path(&state.pm3_dir, "")?;
    Ok(path.display().to_string())
}
