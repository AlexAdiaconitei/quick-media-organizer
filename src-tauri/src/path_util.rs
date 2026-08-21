use std::path::{Component, Path, PathBuf};

pub const APP_FOLDER_NAME: &str = ".quick-media-organizer";
pub const LEGACY_APP_FOLDER_NAME: &str = ".quick-photo-organizer";
pub const IGNORED_FOLDER_NAMES: &[&str] = &["_deleted", APP_FOLDER_NAME, LEGACY_APP_FOLDER_NAME];

pub fn normalize_rel_folder(input: &str) -> String {
    input.trim().replace('\\', "/")
}

pub fn validate_rel_folder(root: &Path, rel: &str) -> Result<String, String> {
    let rel = normalize_rel_folder(rel);
    if rel.is_empty() {
        return Err("Choose a folder first.".into());
    }

    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err("Folder path must be relative to the open album.".into());
    }

    if rel.contains(':') {
        return Err("Invalid folder path.".into());
    }

    for part in rel.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("Folder path cannot contain '..'.".into());
        }
        if IGNORED_FOLDER_NAMES.contains(&part) {
            return Err(format!(
                "Cannot use '{part}' as a destination folder."
            ));
        }
    }

    let dest = root.join(&rel);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve album folder: {e}"))?;
    let canonical_dest = dest
        .canonicalize()
        .or_else(|_| {
            let mut probe = dest.clone();
            while !probe.exists() {
                if !probe.pop() {
                    break;
                }
            }
            if probe.exists() {
                probe.canonicalize()
            } else {
                canonical_root
                    .join(&rel)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| canonical_root.clone())
                    .canonicalize()
            }
        })
        .map_err(|e| format!("Invalid folder path: {e}"))?;

    if !canonical_dest.starts_with(&canonical_root) {
        return Err("Folder path must stay inside the open album.".into());
    }

    Ok(rel)
}

pub fn resolve_dest_dir(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = validate_rel_folder(root, rel)?;
    Ok(root.join(rel))
}

pub fn is_path_inside_root(root: &Path, path: &Path) -> bool {
    let Ok(root_canon) = root.canonicalize() else {
        return false;
    };
    let Ok(path_canon) = path.canonicalize() else {
        return path.starts_with(root);
    };
    path_canon.starts_with(&root_canon)
}

pub fn is_path_in_ignored_dir(root: &Path, path: &Path) -> bool {
    if let Ok(relative) = path.strip_prefix(root) {
        for component in relative.components() {
            if let Component::Normal(name) = component {
                if let Some(name) = name.to_str() {
                    if IGNORED_FOLDER_NAMES.contains(&name) {
                        return true;
                    }
                }
            }
        }
    }
    false
}
