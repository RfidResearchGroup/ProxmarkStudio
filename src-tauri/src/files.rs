use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

const TEXT_LIMIT: usize = 1024 * 1024;
const HEX_LIMIT: usize = 512 * 1024;
const INTERNAL_MARKER: &str = ".pm3-resources.json";

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
pub async fn list_dir(state: State<'_, AppState>, path: String) -> Result<ListDirResult, String> {
    let root = state.pm3_dir.clone();
    tauri::async_runtime::spawn_blocking(move || read_entries(&root, &path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn read_file(
    state: State<'_, AppState>,
    path: String,
    mode: String,
) -> Result<ReadFileResult, String> {
    let root = state.pm3_dir.clone();
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
