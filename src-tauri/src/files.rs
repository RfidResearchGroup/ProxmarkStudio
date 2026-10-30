use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::state::AppState;

const TEXT_LIMIT: usize = 1024 * 1024;
const HEX_LIMIT: usize = 512 * 1024;
const INTERNAL_MARKER: &str = ".pm3-resources.json";
const RESOURCE_DIRS: &[&str] = &[
    "cmdscripts",
    "dictionaries",
    "lualibs",
    "luascripts",
    "pyscripts",
    "resources",
];

fn scope_root(state: &AppState, scope: &str) -> Result<PathBuf, String> {
    match scope {
        "" | "workspace" | "resources" => Ok(state.pm3_dir.clone()),
        other => Err(format!("Unknown file scope: {other}")),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub extension: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListDirResult {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<FileEntry>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileResult {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub binary: bool,
    pub truncated: bool,
    pub mode: String,
    pub content: String,
}

fn modified_seconds(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn normalize_relative(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("Path contains an invalid character.".to_string());
    }
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    for part in trimmed.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("Parent directory traversal is not allowed.".to_string());
        }
        if part.contains(':') {
            return Err("Drive-qualified paths are not allowed.".to_string());
        }
        if part.chars().any(|character| character.is_control()) {
            return Err("Path contains a control character.".to_string());
        }
    }
    Ok(trimmed
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

/// Build a workspace-relative path without touching the filesystem.
pub fn safe_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let normalized = normalize_relative(relative)?;
    let mut result = root.to_path_buf();
    if normalized.is_empty() {
        return Ok(result);
    }
    for part in normalized.split('/') {
        result.push(part);
    }
    if !result.starts_with(root) {
        return Err("Path escapes the Proxmark workspace.".to_string());
    }
    Ok(result)
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Could not create the workspace {}: {error}", root.display()))?;
    root.canonicalize()
        .map_err(|error| format!("Could not resolve the workspace {}: {error}", root.display()))
}

/// Resolve an existing path and make sure symlinks cannot escape the workspace.
pub fn safe_existing_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let candidate = safe_path(root, relative)?;
    let canon_root = canonical_root(root)?;
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    if !canonical.starts_with(&canon_root) {
        return Err("Path escapes the Proxmark workspace.".to_string());
    }
    Ok(canonical)
}

fn safe_destination(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let candidate = safe_path(root, relative)?;
    let canon_root = canonical_root(root)?;
    let parent = candidate
        .parent()
        .ok_or_else(|| "Destination has no parent directory.".to_string())?;
    let canon_parent = parent
        .canonicalize()
        .map_err(|error| format!("{}: {error}", parent.display()))?;
    if !canon_parent.starts_with(&canon_root) {
        return Err("Destination escapes the Proxmark workspace.".to_string());
    }
    Ok(candidate)
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn read_entries(root: &Path, relative: &str) -> Result<ListDirResult, String> {
    let directory = safe_existing_path(root, relative)?;
    if !directory.is_dir() {
        return Err("This path is not a directory.".to_string());
    }
    let canonical_root = canonical_root(root)?;
    let mut entries = Vec::new();

    let read_dir = fs::read_dir(&directory)
        .map_err(|error| format!("Could not read {}: {error}", directory.display()))?;
    for item in read_dir {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        let path = item.path();
        let file_type = match item.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        let name = item.file_name().to_string_lossy().to_string();
        if name == INTERNAL_MARKER {
            continue;
        }
        let metadata = match item.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let is_dir = metadata.is_dir();
        let extension = if is_dir {
            String::new()
        } else {
            name.rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_lowercase())
                .unwrap_or_default()
        };
        let relative_path = relative_string(&canonical_root, &path);
        entries.push(FileEntry {
            name,
            path: relative_path,
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            modified: modified_seconds(&metadata),
            extension,
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let normalized = normalize_relative(relative)?;
    let parent = if normalized.is_empty() {
        None
    } else if let Some((parent, _)) = normalized.rsplit_once('/') {
        Some(parent.to_string())
    } else {
        Some(String::new())
    };

    Ok(ListDirResult {
        path: normalized,
        parent,
        entries,
    })
}

fn hex_dump(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 4);
    for (line, chunk) in bytes.chunks(16).enumerate() {
        let offset = line * 16;
        output.push_str(&format!("{offset:08X}  "));
        for index in 0..16 {
            if let Some(byte) = chunk.get(index) {
                output.push_str(&format!("{byte:02X} "));
            } else {
                output.push_str("   ");
            }
            if index == 7 {
                output.push(' ');
            }
        }
        output.push(' ');
        for byte in chunk {
            output.push(if *byte >= 0x20 && *byte < 0x7f {
                *byte as char
            } else {
                '.'
            });
        }
        output.push('\n');
    }
    output
}

fn read_file_impl(root: &Path, relative: &str, mode: &str) -> Result<ReadFileResult, String> {
    if mode != "text" && mode != "hex" {
        return Err("Unknown preview mode.".to_string());
    }
    let path = safe_existing_path(root, relative)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("Could not stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err("This path is not a file.".to_string());
    }
    let limit = if mode == "text" { TEXT_LIMIT } else { HEX_LIMIT };
    let mut file =
        File::open(&path).map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;

    let sample = &bytes[..bytes.len().min(4096)];
    let binary = sample.iter().any(|byte| *byte == 0);
    let content = if mode == "hex" {
        hex_dump(&bytes)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };

    Ok(ReadFileResult {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: normalize_relative(relative)?,
        size: metadata.len(),
        modified: modified_seconds(&metadata),
        binary,
        truncated: metadata.len() > limit as u64,
        mode: mode.to_string(),
        content,
    })
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("A name is required.".to_string());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("This name is not allowed.".to_string());
    }
    if trimmed
        .chars()
        .any(|character| matches!(character, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || character.is_control())
    {
        return Err("The name contains characters that are not allowed.".to_string());
    }
    Ok(trimmed.to_string())
}

#[tauri::command]
pub async fn list_dir(
    state: State<'_, AppState>,
    path: String,
    scope: String,
) -> Result<ListDirResult, String> {
    let root = scope_root(&state, &scope)?;
    let resource_view = scope == "resources";
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = read_entries(&root, &path)?;
        if resource_view && path.is_empty() {
            result
                .entries
                .retain(|entry| RESOURCE_DIRS.contains(&entry.name.as_str()));
        }
        Ok(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn read_file(
    state: State<'_, AppState>,
    path: String,
    mode: String,
    scope: String,
) -> Result<ReadFileResult, String> {
    let root = scope_root(&state, &scope)?;
    tauri::async_runtime::spawn_blocking(move || read_file_impl(&root, &path, &mode))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let root = state.pm3_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let target = safe_existing_path(&root, &path)?;
        if target == canonical_root(&root)? {
            return Err("The workspace root cannot be deleted.".to_string());
        }
        let metadata = fs::symlink_metadata(&target)
            .map_err(|error| format!("Could not stat {}: {error}", target.display()))?;
        if metadata.is_dir() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("Could not delete {}: {error}", target.display()))
        } else {
            fs::remove_file(&target)
                .map_err(|error| format!("Could not delete {}: {error}", target.display()))
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn move_path(
    state: State<'_, AppState>,
    source: String,
    target: String,
    name: String,
) -> Result<String, String> {
    let root = state.pm3_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let safe_name = validate_name(&name)?;
        let source_path = safe_existing_path(&root, &source)?;
        if source_path == canonical_root(&root)? {
            return Err("The workspace root cannot be moved.".to_string());
        }
        let target_dir = safe_existing_path(&root, &target)?;
        if !target_dir.is_dir() {
            return Err("The destination is not a directory.".to_string());
        }
        if source_path.is_dir() && target_dir.starts_with(&source_path) {
            return Err("A folder cannot be moved inside itself.".to_string());
        }
        let relative_target = if target.is_empty() {
            safe_name.clone()
        } else {
            format!("{}/{}", normalize_relative(&target)?, safe_name)
        };
        let destination = safe_destination(&root, &relative_target)?;
        if destination.exists() {
            return Err("A file or folder with this name already exists.".to_string());
        }
        fs::rename(&source_path, &destination).map_err(|error| {
            format!(
                "Could not move {} to {}: {error}",
                source_path.display(),
                destination.display()
            )
        })?;
        Ok(relative_string(&canonical_root(&root)?, &destination))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    parent: String,
    name: String,
) -> Result<String, String> {
    let root = state.pm3_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let safe_name = validate_name(&name)?;
        let parent_path = safe_existing_path(&root, &parent)?;
        if !parent_path.is_dir() {
            return Err("The parent path is not a directory.".to_string());
        }
        let relative = if parent.is_empty() {
            safe_name
        } else {
            format!("{}/{}", normalize_relative(&parent)?, safe_name)
        };
        let destination = safe_destination(&root, &relative)?;
        if destination.exists() {
            return Err("A file or folder with this name already exists.".to_string());
        }
        fs::create_dir(&destination)
            .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
        Ok(relative)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Deserialize)]
pub struct ImportFile {
    pub name: String,
    pub data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub name: String,
    pub size: u64,
}

#[tauri::command]
pub async fn import_files(
    state: State<'_, AppState>,
    scope: String,
    dir: String,
    files: Vec<ImportFile>,
    overwrite: bool,
) -> Result<ImportResult, String> {
    let root = scope_root(&state, &scope)?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = safe_existing_path(&root, &dir)?;
        if !directory.is_dir() {
            return Err("The import destination is not a directory.".to_string());
        }
        let mut names = Vec::new();
        for file in files {
            let name = validate_name(&file.name)?;
            let relative = if dir.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", normalize_relative(&dir)?, name)
            };
            let destination = safe_path(&root, &relative)?;
            if destination.exists() && !overwrite {
                return Err(format!("{} already exists.", name));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(file.data.as_bytes())
                .map_err(|error| format!("Could not decode {}: {error}", name))?;
            let mut output = File::create(&destination)
                .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
            output
                .write_all(&bytes)
                .map_err(|error| format!("Could not write {}: {error}", destination.display()))?;
            names.push(name);
        }
        Ok(ImportResult {
            imported: names.len(),
            names,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn count_zip_files(directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(directory) else { return 0 };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let Ok(file_type) = entry.file_type() else { return 0 };
            if file_type.is_symlink() {
                0
            } else if file_type.is_dir() {
                count_zip_files(&entry.path())
            } else if file_type.is_file() {
                1
            } else {
                0
            }
        })
        .sum()
}

fn add_zip_dir(
    zip: &mut ZipWriter<File>,
    root: &Path,
    directory: &Path,
    options: SimpleFileOptions,
    app: &tauri::AppHandle,
    done: &mut usize,
    total: usize,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            add_zip_dir(zip, root, &path, options, app, done, total)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let _ = app.emit(
                "zip-progress",
                serde_json::json!({ "done": *done, "total": total, "file": relative }),
            );
            zip.start_file(relative, options).map_err(|error| error.to_string())?;
            let mut input = File::open(&path)
                .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
            std::io::copy(&mut input, zip).map_err(|error| error.to_string())?;
            *done += 1;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn export_workspace_zip(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    destination: Option<String>,
) -> Result<ExportResult, String> {
    let root = state.pm3_dir.clone();
    let cache = state.home.join("cache");
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&cache)
            .map_err(|error| format!("Could not create cache directory: {error}"))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or(0);
        let name = format!("proxmark-studio-workspace-{stamp}.zip");
        let output_path = if let Some(directory) = destination.filter(|value| !value.trim().is_empty()) {
            let directory = PathBuf::from(directory);
            if !directory.is_dir() {
                return Err("The selected export directory does not exist.".to_string());
            }
            directory.join(&name)
        } else {
            cache.join(&name)
        };
        let file = File::create(&output_path)
            .map_err(|error| format!("Could not create {}: {error}", output_path.display()))?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let total = count_zip_files(&root);
        let _ = app.emit(
            "zip-progress",
            serde_json::json!({ "done": 0, "total": total, "file": "" }),
        );
        let mut done = 0usize;
        add_zip_dir(&mut zip, &root, &root, options, &app, &mut done, total)?;
        let file = zip.finish().map_err(|error| error.to_string())?;
        let size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let _ = app.emit(
            "zip-progress",
            serde_json::json!({ "done": done, "total": total, "file": "" }),
        );
        Ok(ExportResult {
            path: output_path.to_string_lossy().to_string(),
            name,
            size,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn pick_export_directory() -> Result<Option<String>, String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        return tauri::async_runtime::spawn_blocking(|| {
            rfd::FileDialog::new()
                .pick_folder()
                .map(|path| path.to_string_lossy().to_string())
        })
        .await
        .map_err(|error| error.to_string());
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        Ok(None)
    }
}

/// Write a file inside the workspace. Used by the resource installer for the
/// marker file and intentionally not exposed as a Tauri command.
pub fn write_workspace_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), String> {
    let destination = safe_destination(root, relative)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    let mut file = File::create(&destination)
        .map_err(|error| format!("Could not write {}: {error}", destination.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("Could not write {}: {error}", destination.display()))
}
