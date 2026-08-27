use crate::models::RenameMode;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn sanitize_base_name(input: &str) -> String {
    let mut result = input.trim().to_string();
    for ch in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        result = result.replace(ch, "");
    }
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().trim_end_matches('.').to_string()
}

pub struct SanitizeResult {
    pub sanitized: String,
    pub was_modified: bool,
}

pub fn sanitize_with_feedback(input: &str) -> SanitizeResult {
    let trimmed = input.trim();
    let sanitized = sanitize_base_name(input);
    let was_modified = !trimmed.is_empty() && sanitized != trimmed;
    SanitizeResult {
        sanitized,
        was_modified,
    }
}

pub fn format_filename(base: &str, ext: &str) -> String {
    if ext.is_empty() {
        base.to_string()
    } else {
        format!("{base}.{ext}")
    }
}

fn counter_key(dir: &Path, base: &str) -> String {
    format!("{}|{}", dir.to_string_lossy(), base)
}

pub fn resolve_group_names(
    dir: &Path,
    base: &str,
    extensions: &[String],
    rename_mode: RenameMode,
    counter_map: &mut HashMap<String, u32>,
) -> Option<Vec<String>> {
    let base = sanitize_base_name(base);
    if base.is_empty() || extensions.is_empty() {
        return None;
    }

    match rename_mode {
        RenameMode::Free => {
            let plain: Vec<String> = extensions
                .iter()
                .map(|ext| format_filename(&base, ext))
                .collect();
            let key = counter_key(dir, &base);
            if plain.iter().all(|name| !dir.join(name).exists()) && !counter_map.contains_key(&key)
            {
                counter_map.insert(key, 0);
                return Some(plain);
            }

            let counter = counter_map.entry(key).or_insert(0);
            loop {
                *counter += 1;
                let suffixed_base = format!("{base}_{counter:03}");
                let candidates: Vec<String> = extensions
                    .iter()
                    .map(|ext| format_filename(&suffixed_base, ext))
                    .collect();
                if candidates.iter().all(|name| !dir.join(name).exists()) {
                    return Some(candidates);
                }
            }
        }
        RenameMode::PrefixCounter => {
            let key = counter_key(dir, &base);
            let counter = counter_map.entry(key).or_insert(0);
            loop {
                *counter += 1;
                let suffixed_base = format!("{base}_{counter:03}");
                let candidates: Vec<String> = extensions
                    .iter()
                    .map(|ext| format_filename(&suffixed_base, ext))
                    .collect();
                if candidates.iter().all(|name| !dir.join(name).exists()) {
                    return Some(candidates);
                }
            }
        }
    }
}

pub fn resolve_group_original_names(dir: &Path, sources: &[PathBuf]) -> Vec<String> {
    let originals: Vec<String> = sources
        .iter()
        .filter_map(|source| {
            source
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    if originals.is_empty() {
        return vec!["file".to_string()];
    }

    if originals.iter().all(|name| !dir.join(name).exists()) {
        return originals;
    }

    let primary = &sources[0];
    let original_name = primary
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let stamped = resolve_trash_name(dir, &original_name);
    let stem = PathBuf::from(&stamped)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();

    sources
        .iter()
        .map(|source| {
            let ext = source
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            format_filename(&stem, &ext)
        })
        .collect()
}

pub fn resolve_trash_name(deleted_dir: &Path, original_name: &str) -> String {
    let mut candidate = original_name.to_string();
    if !deleted_dir.join(&candidate).exists() {
        return candidate;
    }

    let path = PathBuf::from(original_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    candidate = format!("{stem}_{stamp}{ext}");

    let mut suffix = 1;
    while deleted_dir.join(&candidate).exists() {
        candidate = format!("{stem}_{stamp}_{suffix}{ext}");
        suffix += 1;
    }

    candidate
}

pub fn resolve_group_trash_names(deleted_dir: &Path, sources: &[PathBuf]) -> Vec<String> {
    let primary = sources
        .first()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let stamped = resolve_trash_name(deleted_dir, &primary);
    let stem = PathBuf::from(&stamped)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();

    sources
        .iter()
        .map(|source| {
            let ext = source
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let plain = format_filename(&stem, &ext);
            if !deleted_dir.join(&plain).exists() {
                plain
            } else {
                resolve_trash_name(deleted_dir, &plain)
            }
        })
        .collect()
}

pub fn extensions_for_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase()
        })
        .collect()
}
