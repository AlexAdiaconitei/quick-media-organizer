use crate::models::{MediaItem, MediaKind, SortMode};
use crate::path_util::{is_path_in_ignored_dir, IGNORED_FOLDER_NAMES};
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "tiff", "tif",
];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "3gp"];
const IGNORED_DIRS: &[&str] = IGNORED_FOLDER_NAMES;

pub fn is_media_extension(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    IMAGE_EXTENSIONS.contains(&ext.as_str()) || VIDEO_EXTENSIONS.contains(&ext.as_str())
}

pub fn is_video_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Skips an ignored folder *and everything under it*, so the app's own
/// `.quick-media-organizer/trim-backups` never shows up as a destination.
fn keeps_walking(entry: &walkdir::DirEntry) -> bool {
    entry.depth() == 0
        || !entry
            .file_name()
            .to_str()
            .is_some_and(|name| IGNORED_DIRS.contains(&name))
}

pub fn list_subfolders(root: &Path) -> Vec<String> {
    let mut folders = Vec::new();

    if !root.is_dir() {
        return folders;
    }

    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(keeps_walking)
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        if let Ok(relative) = entry.path().strip_prefix(root) {
            folders.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    folders.sort();
    folders.dedup();
    folders
}

/// Media the queue is missing while "include subfolders" is off: everything
/// below `root` at any depth, skipping the app's own folders.
pub fn count_root_subfolder_media(root: &Path) -> usize {
    WalkDir::new(root)
        .min_depth(2)
        .into_iter()
        .filter_entry(keeps_walking)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_supported_file(entry.path()))
        .count()
}

pub fn scan_folder(root: &Path, recursive: bool) -> Result<Vec<MediaItem>, String> {
    let mut files: Vec<PathBuf> = Vec::new();

    if recursive {
        for entry in WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if is_path_in_ignored_dir(root, path) {
                continue;
            }

            if is_supported_file(path) {
                files.push(path.to_path_buf());
            }
        }
    } else {
        // Reported rather than swallowed: an unreadable folder must not look
        // like an empty one.
        let entries =
            fs::read_dir(root).map_err(|e| format!("Cannot read {}: {e}", root.display()))?;
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && is_supported_file(&path) {
                files.push(path);
            }
        }
    }

    let mut grouped = group_live_photos(&files);
    sort_items(&mut grouped, SortMode::ExifDate);
    Ok(grouped)
}

pub fn build_media_item_from_paths(path_strs: &[String]) -> MediaItem {
    let paths: Vec<PathBuf> = path_strs.iter().map(PathBuf::from).collect();
    let kind = if paths.len() >= 2 && detect_live_photo_pair(&paths).is_some() {
        MediaKind::LivePhoto
    } else {
        MediaKind::Single
    };
    build_media_item_fast(&paths, kind)
}

pub fn enrich_exif_dates_for_sort(items: &mut [MediaItem]) {
    items.par_iter_mut().for_each(|item| {
        if item.is_video || item.exif_date.is_some() {
            return;
        }
        let primary = PathBuf::from(&item.paths[0]);
        item.exif_date = read_exif_date_only(&primary);
    });
}

pub fn prepare_sorted_items(items: &mut [MediaItem], mode: SortMode) {
    if mode == SortMode::ExifDate {
        enrich_exif_dates_for_sort(items);
    }
    sort_items(items, mode);
}

pub fn enrich_item_metadata(item: &mut MediaItem) {
    if item.exif_date.is_some() && item.width.is_some() && item.height.is_some() {
        return;
    }

    let primary = PathBuf::from(&item.paths[0]);
    if item.is_video {
        return;
    }

    let exif = read_exif_fields(&primary);
    if item.exif_date.is_none() {
        item.exif_date = exif.exif_date;
    }
    if item.width.is_none() {
        item.width = exif.width;
    }
    if item.height.is_none() {
        item.height = exif.height;
    }
}

pub fn refresh_item_size(item: &mut MediaItem) {
    item.size_bytes = item
        .paths
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|meta| meta.len())
        .sum();
}

pub fn diagnose_media_file(path: &Path) -> crate::models::MediaFileDiagnosis {
    let size_bytes = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);

    if size_bytes == 0 {
        return crate::models::MediaFileDiagnosis {
            issue: "empty".into(),
            size_bytes,
        };
    }

    let mut header = [0u8; 16];
    let read_len = File::open(path)
        .and_then(|mut file| file.read(&mut header))
        .unwrap_or(0);

    if read_len < 4 {
        return crate::models::MediaFileDiagnosis {
            issue: "too_small".into(),
            size_bytes,
        };
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if matches!(extension.as_str(), "jpg" | "jpeg") && !looks_like_jpeg(&header) {
        return crate::models::MediaFileDiagnosis {
            issue: "content_mismatch".into(),
            size_bytes,
        };
    }

    if extension == "png" && header[0..4] != [0x89, 0x50, 0x4E, 0x47] {
        return crate::models::MediaFileDiagnosis {
            issue: "content_mismatch".into(),
            size_bytes,
        };
    }

    if matches!(extension.as_str(), "heic" | "heif") && !looks_like_heif(&header) {
        return crate::models::MediaFileDiagnosis {
            issue: "content_mismatch".into(),
            size_bytes,
        };
    }

    crate::models::MediaFileDiagnosis {
        issue: "unknown".into(),
        size_bytes,
    }
}

fn looks_like_jpeg(header: &[u8]) -> bool {
    header.len() >= 3 && header[0] == 0xFF && header[1] == 0xD8 && header[2] == 0xFF
}

fn looks_like_heif(header: &[u8]) -> bool {
    header.len() >= 12 && &header[4..8] == b"ftyp"
}

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(is_media_extension)
}

fn group_key(path: &Path) -> String {
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    format!("{parent}|{stem}")
}

fn group_live_photos(files: &[PathBuf]) -> Vec<MediaItem> {
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for path in files {
        groups
            .entry(group_key(path))
            .or_default()
            .push(path.clone());
    }

    let mut items = Vec::new();
    let mut seen_groups = std::collections::HashSet::new();

    for path in files {
        let key = group_key(path);
        if !seen_groups.insert(key.clone()) {
            continue;
        }

        let group = groups.remove(&key).unwrap_or_default();
        let pair = detect_live_photo_pair(&group);
        if let Some(pair) = pair {
            items.push(build_media_item_fast(&pair, MediaKind::LivePhoto));
            let paired: std::collections::HashSet<_> = pair.iter().collect();
            for candidate in group {
                if !paired.contains(&candidate) {
                    items.push(build_media_item_fast(&[candidate], MediaKind::Single));
                }
            }
        } else {
            for candidate in group {
                items.push(build_media_item_fast(&[candidate], MediaKind::Single));
            }
        }
    }

    items
}

fn detect_live_photo_pair(group: &[PathBuf]) -> Option<Vec<PathBuf>> {
    if group.len() < 2 {
        return None;
    }

    let mut image: Option<PathBuf> = None;
    let mut video: Option<PathBuf> = None;

    for path in group {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if is_video_extension(&ext) {
            video = Some(path.clone());
        } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            image = Some(path.clone());
        }
    }

    match (image, video) {
        (Some(img), Some(vid)) => Some(vec![img, vid]),
        _ => None,
    }
}

fn build_media_item_fast(paths: &[PathBuf], kind: MediaKind) -> MediaItem {
    let primary = &paths[0];
    let file_name = primary
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let extension = primary
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut modified_at = None;
    let mut size_bytes = 0u64;
    for path in paths {
        if let Ok(meta) = fs::metadata(path) {
            size_bytes += meta.len();
            if path == primary {
                modified_at = meta.modified().ok().map(|t| {
                    DateTime::<Utc>::from(t)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                });
            }
        }
    }

    MediaItem {
        id: primary.to_string_lossy().to_string(),
        paths: paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        file_name,
        extension: extension.clone(),
        exif_date: None,
        modified_at,
        size_bytes,
        is_video: is_video_extension(&extension),
        kind,
        width: None,
        height: None,
    }
}

struct ExifFields {
    exif_date: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

fn read_exif_date_only(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .ok()?;
    exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .map(|field| field.display_value().to_string())
}

fn read_exif_fields(path: &Path) -> ExifFields {
    let mut exif_date = None;
    let mut width = None;
    let mut height = None;

    if let Ok(file) = fs::File::open(path) {
        if let Ok(exif) =
            exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file))
        {
            if let Some(field) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
                exif_date = Some(field.display_value().to_string());
            }
            if let Some(field) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
                width = field.value.get_uint(0);
            }
            if let Some(field) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
                height = field.value.get_uint(0);
            }
        }
    }

    ExifFields {
        exif_date,
        width,
        height,
    }
}

fn sort_date(item: &MediaItem) -> Option<&String> {
    item.exif_date.as_ref().or(item.modified_at.as_ref())
}

pub fn sort_items(items: &mut [MediaItem], mode: SortMode) {
    items.sort_by(|a, b| match mode {
        SortMode::FileName => a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()),
        SortMode::ModifiedDate => a
            .modified_at
            .cmp(&b.modified_at)
            .then(a.file_name.cmp(&b.file_name)),
        SortMode::ExifDate => sort_date(a)
            .cmp(&sort_date(b))
            .then(a.file_name.cmp(&b.file_name)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn groups_live_photos_within_same_folder_only() {
        let files = vec![
            PathBuf::from("/album/a/photo.jpg"),
            PathBuf::from("/album/b/photo.jpg"),
            PathBuf::from("/album/a/photo.mp4"),
        ];

        let items = group_live_photos(&files);

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| item.kind == MediaKind::LivePhoto));
        assert!(items.iter().any(|item| {
            item.kind == MediaKind::Single && item.paths[0].ends_with("/album/b/photo.jpg")
        }));
    }

    #[test]
    fn emits_all_files_when_group_has_extra_stem_siblings() {
        let files = vec![
            PathBuf::from("/album/photo.jpg"),
            PathBuf::from("/album/photo.mp4"),
            PathBuf::from("/album/photo (1).jpg"),
        ];

        let items = group_live_photos(&files);

        assert_eq!(items.len(), 2);
        let primaries: Vec<_> = items.iter().map(|item| item.paths[0].clone()).collect();
        assert!(primaries.iter().any(|path| path.ends_with("photo.jpg")));
        assert!(primaries.iter().any(|path| path.ends_with("photo (1).jpg")));
    }
}
