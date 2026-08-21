use crate::fs_util::{apply_timestamps, execute_moves, move_file_preserve, read_timestamps};
use crate::media::{
    build_media_item_from_paths, count_root_subfolder_media, enrich_item_metadata,
    list_subfolders, prepare_sorted_items, refresh_item_size, scan_folder, sort_items,
};
use crate::models::{
    ActionResult, AppSettings, FrontendState, LayoutMode, MediaItem, PathPair,
    RenameMode, SessionData, SessionStats, SortMode, UndoAction, UndoStatKind,
};
use crate::path_util::{resolve_dest_dir, validate_rel_folder};
use crate::rename::{
    extensions_for_paths, resolve_group_names, resolve_group_original_names,
    resolve_group_trash_names, sanitize_with_feedback,
};
use crate::session::{load_app_settings, load_session, save_app_settings, save_session};
use crate::video::{trim_backup_path, FfmpegTools};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_UNDO: usize = 100;

pub struct AppState {
    pub folder_path: Option<PathBuf>,
    pub items: Vec<MediaItem>,
    pub current_index: usize,
    pub sort_mode: SortMode,
    pub scan_recursive: bool,
    pub rename_mode: RenameMode,
    pub counter_map: HashMap<String, u32>,
    pub undo_stack: Vec<UndoAction>,
    pub recent_folders: Vec<String>,
    pub armed_folder: Option<String>,
    pub stats: SessionStats,
    pub app_settings: AppSettings,
    pub app_data_dir: PathBuf,
    undo_overflow_notified: bool,
    session_complete: bool,
    processed_paths: HashSet<String>,
    transient_session_reset: bool,
    transient_resume_from: Option<usize>,
    transient_subfolder_media: Option<usize>,
    cached_subfolders: Vec<String>,
    subfolders_dirty: bool,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let app_settings = load_app_settings(&app_data_dir);
        Self {
            folder_path: None,
            items: Vec::new(),
            current_index: 0,
            sort_mode: SortMode::default(),
            scan_recursive: false,
            rename_mode: RenameMode::default(),
            counter_map: HashMap::new(),
            undo_stack: Vec::new(),
            recent_folders: Vec::new(),
            armed_folder: None,
            stats: SessionStats::default(),
            app_settings,
            app_data_dir,
            undo_overflow_notified: false,
            session_complete: false,
            processed_paths: HashSet::new(),
            transient_session_reset: false,
            transient_resume_from: None,
            transient_subfolder_media: None,
            cached_subfolders: Vec::new(),
            subfolders_dirty: true,
        }
    }

    pub fn open_folder(&mut self, folder: PathBuf) -> Result<(), String> {
        self.transient_session_reset = false;
        self.transient_resume_from = None;
        self.transient_subfolder_media = None;

        let session = load_session(&folder);
        let mut saved_paths: Vec<String> = Vec::new();

        if let Some(session) = &session {
            self.sort_mode = session.sort_mode;
            self.scan_recursive = session.scan_recursive;
            self.rename_mode = session.rename_mode;
            saved_paths = session.current_item_paths.clone();
        } else {
            self.counter_map.clear();
            self.undo_stack.clear();
            self.stats = SessionStats::default();
            self.current_index = 0;
            self.armed_folder = None;
            self.recent_folders.clear();
            self.processed_paths.clear();
        }

        let mut items = scan_folder(&folder, self.scan_recursive)?;
        if items.is_empty() && !self.scan_recursive {
            let recursive_items = scan_folder(&folder, true)?;
            if !recursive_items.is_empty() {
                self.scan_recursive = true;
                items = recursive_items;
            }
        }
        if items.is_empty() {
            return Err("No photos or videos found in this folder.".into());
        }

        let subfolder_media = count_root_subfolder_media(&folder);

        if let Some(session) = session {
            self.counter_map = session.counter_map;
            if session.stats.moved == 0 && !session.recent_folders.is_empty() {
                // Sessions saved before recents were scoped per album could inherit
                // another folder's list; drop them until the user saves here.
                self.recent_folders.clear();
            } else {
                self.recent_folders = session.recent_folders;
            }
            self.armed_folder = session.armed_folder;
            self.undo_stack = session.undo_stack;
            self.stats = session.stats;
            self.processed_paths = session.processed_paths.into_iter().collect();
            if saved_paths.is_empty() {
                self.current_index = session
                    .current_index
                    .min(items.len().saturating_sub(1));
            }
        }

        prepare_sorted_items(&mut items, self.sort_mode);
        if !saved_paths.is_empty() {
            if let Some(idx) = self.find_item_index_by_paths(&saved_paths, &items) {
                self.current_index = idx;
            } else if let Some(idx) = session_index_fallback(&saved_paths, &items) {
                self.current_index = idx;
            } else {
                // Last saved file is gone (renamed outside the app, deleted, etc.)
                self.current_index = 0;
                self.transient_session_reset = true;
            }
        }

        self.folder_path = Some(folder.clone());
        self.items = items;
        self.sanitize_inflated_session_stats();
        if let Some(item) = self.current_item() {
            if self.is_item_processed(item) {
                let start = (self.current_index + 1).min(self.items.len().saturating_sub(1));
                if let Some(next) = self.find_next_unprocessed_from(start) {
                    self.current_index = next;
                } else if let Some(next) = self.find_next_unprocessed_from(0) {
                    self.current_index = next;
                }
            }
        }

        if !self.scan_recursive && subfolder_media > 0 {
            self.transient_subfolder_media = Some(subfolder_media);
        }

        if self.current_index > 0 && !self.transient_session_reset {
            self.transient_resume_from = Some(self.current_index + 1);
        }

        self.session_complete = false;
        self.app_settings.last_folder_path = Some(folder.to_string_lossy().to_string());
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        self.subfolders_dirty = true;
        self.refresh_subfolders_cache();
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn to_frontend_state(&mut self) -> FrontendState {
        if let Some(item) = self.items.get_mut(self.current_index) {
            enrich_item_metadata(item);
        }

        let folder_path = self.folder_path.as_ref().map(|p| p.to_string_lossy().to_string());
        if self.subfolders_dirty {
            self.refresh_subfolders_cache();
        }

        FrontendState {
            folder_path,
            current_index: self.current_index,
            total: self.items.len(),
            item: self.current_item().cloned(),
            sort_mode: self.sort_mode,
            scan_recursive: self.scan_recursive,
            rename_mode: self.rename_mode,
            armed_folder: self.armed_folder.clone(),
            recent_folders: self.recent_folders.clone(),
            favorite_folders: self.app_settings.favorite_folders.clone(),
            existing_subfolders: self.cached_subfolders.clone(),
            stats: self.stats.clone(),
            session_complete: self.session_complete,
            session_reset: false,
            resume_from: None,
            subfolder_media_count: None,
        }
    }

    pub fn take_transient_open_notices(&mut self) -> (bool, Option<usize>, Option<usize>) {
        let reset = self.transient_session_reset;
        self.transient_session_reset = false;
        (
            reset,
            self.transient_resume_from.take(),
            self.transient_subfolder_media.take(),
        )
    }

    pub fn current_item(&self) -> Option<&MediaItem> {
        self.items.get(self.current_index)
    }

    pub fn skip(&mut self, delta: i32) -> Result<(), String> {
        if self.items.is_empty() {
            return Ok(());
        }
        let len = self.items.len() as i32;

        if delta > 0 {
            if self.current_index as i32 >= len - 1 {
                self.session_complete = true;
                self.armed_folder = None;
                self.persist_session_best_effort();
                return Ok(());
            }

            let next = (self.current_index as i32 + 1) as usize;
            self.stats.skipped += 1;
            self.current_index = next;
        } else if delta < 0 {
            let next = (self.current_index as i32 + delta).max(0) as usize;
            if next != self.current_index {
                self.current_index = next;
            }
        }

        self.session_complete = false;
        self.armed_folder = None;
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn dismiss_session_complete(&mut self) -> Result<(), String> {
        self.session_complete = false;
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn restart_queue(&mut self) -> Result<(), String> {
        let folder = self.folder_path.clone().ok_or("No folder open")?;
        self.items = scan_folder(&folder, self.scan_recursive)?;
        prepare_sorted_items(&mut self.items, self.sort_mode);
        self.current_index = 0;
        self.session_complete = false;
        self.armed_folder = None;
        self.stats = SessionStats::default();
        self.counter_map.clear();
        self.processed_paths.clear();
        self.subfolders_dirty = true;
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn rename_current(&mut self, name: &str) -> Result<ActionResult, String> {
        let folder = self.folder_path.clone().ok_or("No folder open")?;
        if self.armed_folder.is_some() {
            return self.move_current_to_folder(self.armed_folder.clone(), Some(name.to_string()));
        }

        let feedback = sanitize_with_feedback(name);
        if feedback.sanitized.is_empty() {
            return Ok(self.fail("Write a name before pressing Enter."));
        }

        let item = self
            .current_item()
            .cloned()
            .ok_or("No media item selected")?;

        let old_index = self.current_index;
        self.mark_item_processed(&item);

        let sources: Vec<PathBuf> = item.paths.iter().map(PathBuf::from).collect();
        let extensions = extensions_for_paths(&sources);
        let dest_names = resolve_group_names(
            &folder,
            &feedback.sanitized,
            &extensions,
            self.rename_mode,
            &mut self.counter_map,
        )
        .ok_or("Invalid name")?;

        let completed = execute_moves(&sources, &dest_names, &folder)?;
        let new_paths: Vec<String> = completed
            .iter()
            .map(|(dest, _)| dest.to_string_lossy().to_string())
            .collect();
        self.mark_paths_processed(&new_paths);
        let undo_moves = completed
            .into_iter()
            .map(|(dest, source)| PathPair {
                from: dest.to_string_lossy().to_string(),
                to: source.to_string_lossy().to_string(),
            })
            .collect();

        let mut message = "Renamed".to_string();
        if feedback.was_modified {
            message = format!("Renamed (adjusted to \"{}\")", feedback.sanitized);
        }

        self.push_undo(UndoAction::Rename {
            moves: undo_moves,
            focus_paths: item.paths.clone(),
            stat_kind: UndoStatKind::Renamed,
        });
        self.stats.renamed += 1;
        self.refresh_after_rename(old_index, &new_paths)?;
        Ok(self.ok(&message))
    }

    pub fn trash_current(&mut self) -> Result<ActionResult, String> {
        let folder = self.folder_path.clone().ok_or("No folder open")?;
        let item = self
            .current_item()
            .cloned()
            .ok_or("No media item selected")?;

        self.mark_item_processed(&item);

        let deleted_dir = folder.join("_deleted");
        std::fs::create_dir_all(&deleted_dir).map_err(|e| e.to_string())?;

        let sources: Vec<PathBuf> = item.paths.iter().map(PathBuf::from).collect();
        let dest_names = resolve_group_trash_names(&deleted_dir, &sources);
        let completed = execute_moves(&sources, &dest_names, &deleted_dir)?;
        let undo_moves = completed
            .into_iter()
            .map(|(dest, source)| PathPair {
                from: dest.to_string_lossy().to_string(),
                to: source.to_string_lossy().to_string(),
            })
            .collect();

        self.push_undo(UndoAction::Trash {
            moves: undo_moves,
            focus_paths: item.paths.clone(),
            stat_kind: UndoStatKind::Trashed,
        });
        self.stats.trashed += 1;
        self.remove_current_from_queue()?;
        Ok(self.ok("Moved to _deleted (not system Trash). Press Undo to restore."))
    }

    pub fn move_current_to_folder(
        &mut self,
        folder_rel: Option<String>,
        name: Option<String>,
    ) -> Result<ActionResult, String> {
        let root = self.folder_path.clone().ok_or("No folder open")?;
        let rel = folder_rel
            .or_else(|| self.armed_folder.clone())
            .ok_or("Choose a folder first")?;
        let rel = validate_rel_folder(&root, &rel)?;
        let dest_dir = resolve_dest_dir(&root, &rel)?;
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

        let item = self
            .current_item()
            .cloned()
            .ok_or("No media item selected")?;

        self.mark_item_processed(&item);

        let sources: Vec<PathBuf> = item.paths.iter().map(PathBuf::from).collect();
        let extensions = extensions_for_paths(&sources);

        let dest_names = if let Some(ref raw_name) = name {
            let feedback = sanitize_with_feedback(raw_name);
            if feedback.sanitized.is_empty() {
                resolve_group_original_names(&dest_dir, &sources)
            } else {
                resolve_group_names(
                    &dest_dir,
                    &feedback.sanitized,
                    &extensions,
                    self.rename_mode,
                    &mut self.counter_map,
                )
                .ok_or("Invalid name")?
            }
        } else {
            resolve_group_original_names(&dest_dir, &sources)
        };

        let completed = execute_moves(&sources, &dest_names, &dest_dir)?;
        let undo_moves = completed
            .into_iter()
            .map(|(dest, source)| PathPair {
                from: dest.to_string_lossy().to_string(),
                to: source.to_string_lossy().to_string(),
            })
            .collect();

        self.remember_folder(&rel);
        self.mark_subfolders_dirty();
        self.push_undo(UndoAction::MoveToFolder {
            moves: undo_moves,
            focus_paths: item.paths.clone(),
            stat_kind: UndoStatKind::Moved,
        });
        self.stats.moved += 1;
        self.armed_folder = None;
        self.remove_current_from_queue()?;
        Ok(self.ok("Saved to folder"))
    }

    pub fn trim_current_video(&mut self, trim_start: f64, trim_end: f64) -> Result<ActionResult, String> {
        let folder = self.folder_path.clone().ok_or("No folder open")?;
        let item = self
            .current_item()
            .cloned()
            .ok_or("No media item selected")?;

        if !item.is_video {
            return Ok(self.fail("Current item is not a video."));
        }

        let video_path = PathBuf::from(&item.paths[0]);
        let tools = FfmpegTools::locate()?;
        let duration = tools.probe_duration(&video_path)?;

        let trim_start = trim_start.max(0.0);
        let trim_end = trim_end.min(duration);

        if trim_end <= trim_start + 0.05 {
            return Ok(self.fail("Trim range is too short. Move the start/end markers apart."));
        }

        if trim_start < 0.05 && (duration - trim_end) < 0.05 {
            return Ok(self.fail("Nothing to trim — the full video is selected."));
        }

        let focus_paths = item.paths.clone();
        let snap = read_timestamps(&video_path).map_err(|e| e.to_string())?;

        let backup_path = trim_backup_path(&folder, &video_path);
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&video_path, &backup_path).map_err(|e| e.to_string())?;

        let ext = video_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4");
        let temp_path = video_path.with_file_name(format!(
            ".trim-tmp-{}",
            video_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("video")
        ));
        let temp_path = temp_path.with_extension(ext);

        if let Err(err) = tools.trim_lossless(&video_path, &temp_path, trim_start, trim_end) {
            let _ = std::fs::remove_file(&backup_path);
            let _ = std::fs::remove_file(&temp_path);
            return Err(err);
        }

        if let Err(err) = std::fs::remove_file(&video_path) {
            let _ = std::fs::remove_file(&temp_path);
            let _ = std::fs::remove_file(&backup_path);
            return Err(err.to_string());
        }

        if let Err(err) = std::fs::rename(&temp_path, &video_path) {
            let _ = std::fs::copy(&backup_path, &video_path);
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!("Failed to replace video file: {err}"));
        }

        apply_timestamps(&video_path, &snap).map_err(|e| e.to_string())?;

        self.push_undo(UndoAction::TrimVideo {
            moves: vec![PathPair {
                from: backup_path.to_string_lossy().to_string(),
                to: video_path.to_string_lossy().to_string(),
            }],
            focus_paths,
            stat_kind: UndoStatKind::None,
        });

        if let Some(item) = self.items.get_mut(self.current_index) {
            refresh_item_size(item);
        }
        self.persist_session_best_effort();
        Ok(self.ok("Video trimmed losslessly (no re-encoding)"))
    }

    pub fn undo_last(&mut self) -> Result<ActionResult, String> {
        let action = self
            .undo_stack
            .last()
            .cloned()
            .ok_or("Nothing to undo")?;

        let (moves, focus_paths, stat_kind) = match &action {
            UndoAction::Rename {
                moves,
                focus_paths,
                stat_kind,
            }
            | UndoAction::Trash {
                moves,
                focus_paths,
                stat_kind,
            }
            | UndoAction::MoveToFolder {
                moves,
                focus_paths,
                stat_kind,
            }
            | UndoAction::FlattenToRoot {
                moves,
                focus_paths,
                stat_kind,
            }
            | UndoAction::TrimVideo {
                moves,
                focus_paths,
                stat_kind,
            }
            | UndoAction::ConvertMedia {
                moves,
                focus_paths,
                stat_kind,
                ..
            } => (moves.clone(), focus_paths.clone(), *stat_kind),
        };

        // A conversion that changed the extension left a second file behind
        // (a.heic -> a.jpg); remove it before restoring the backup.
        if let UndoAction::ConvertMedia { remove_paths, .. } = &action {
            for path in remove_paths {
                let _ = std::fs::remove_file(path);
            }
        }

        let mut reverted: Vec<PathPair> = Vec::new();
        for pair in &moves {
            match move_file_preserve(Path::new(&pair.from), Path::new(&pair.to)) {
                Ok(()) => reverted.push(pair.clone()),
                Err(err) => {
                    for done in reverted.iter().rev() {
                        let _ = move_file_preserve(Path::new(&done.to), Path::new(&done.from));
                    }
                    return Err(format!("Undo failed: {err}"));
                }
            }
        }

        self.undo_stack.pop();
        if !matches!(action, UndoAction::FlattenToRoot { .. }) {
            self.revert_stat(stat_kind);
        }
        self.unmark_paths(&focus_paths);
        for pair in &moves {
            self.unmark_path(&pair.from);
            self.unmark_path(&pair.to);
        }
        self.session_complete = false;
        if matches!(action, UndoAction::FlattenToRoot { .. }) {
            self.stats = SessionStats::default();
            self.processed_paths.clear();
        }
        self.apply_undo_to_queue(&action, &focus_paths, &moves);
        self.persist_session_best_effort();
        Ok(self.ok("Undone"))
    }

    fn find_item_index_by_paths(&self, paths: &[String], items: &[MediaItem]) -> Option<usize> {
        if paths.is_empty() {
            return None;
        }
        items.iter().position(|item| item.paths == paths).or_else(|| {
            items.iter().position(|item| {
                paths
                    .iter()
                    .any(|focus| item.paths.iter().any(|path| path == focus))
            })
        })
    }

    fn mark_item_processed(&mut self, item: &MediaItem) {
        self.processed_paths.insert(item.id.clone());
        self.mark_paths_processed(&item.paths);
    }

    fn mark_paths_processed(&mut self, paths: &[String]) {
        for path in paths {
            self.processed_paths.insert(path.clone());
        }
    }

    fn unmark_path(&mut self, path: &str) {
        self.processed_paths.remove(path);
    }

    fn unmark_paths(&mut self, paths: &[String]) {
        for path in paths {
            self.unmark_path(path);
        }
    }

    fn is_item_processed(&self, item: &MediaItem) -> bool {
        self.processed_paths.contains(&item.id)
            || item.paths.iter().any(|path| self.processed_paths.contains(path))
    }

    fn find_next_unprocessed_from(&self, start: usize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }

        let start = start.min(self.items.len().saturating_sub(1));
        for index in start..self.items.len() {
            if !self.is_item_processed(&self.items[index]) {
                return Some(index);
            }
        }
        None
    }

    fn advance_after_processing(&mut self, from_index: usize, step_forward: bool) {
        if self.items.is_empty() {
            self.current_index = 0;
            self.session_complete = true;
            return;
        }

        let start = if step_forward {
            (from_index + 1).min(self.items.len().saturating_sub(1))
        } else {
            from_index.min(self.items.len().saturating_sub(1))
        };

        if let Some(next) = self.find_next_unprocessed_from(start) {
            self.current_index = next;
            self.session_complete = false;
            return;
        }

        if let Some(next) = self.find_next_unprocessed_from(0) {
            self.current_index = next;
            self.session_complete = false;
        } else {
            self.session_complete = true;
        }
    }

    /// Full queue, for the batch selection grid.
    pub fn list_items(&self) -> Vec<MediaItem> {
        self.items.clone()
    }

    /// Registers an in-place batch conversion so it can be undone, and rebuilds
    /// the queue (names and extensions may have changed).
    pub fn apply_batch_replacements(
        &mut self,
        replacements: &[(String, String, String)],
    ) -> Result<usize, String> {
        if replacements.is_empty() {
            if let Some(item) = self.items.get_mut(self.current_index) {
                refresh_item_size(item);
            }
            return Ok(0);
        }

        let mut moves: Vec<PathPair> = Vec::new();
        let mut focus_paths: Vec<String> = Vec::new();
        let mut remove_paths: Vec<String> = Vec::new();

        for (backup, original, converted) in replacements {
            moves.push(PathPair {
                from: backup.clone(),
                to: original.clone(),
            });
            focus_paths.push(original.clone());
            if converted != original {
                remove_paths.push(converted.clone());
            }
        }

        let count = moves.len();
        self.push_undo(UndoAction::ConvertMedia {
            moves,
            focus_paths: focus_paths.clone(),
            stat_kind: UndoStatKind::None,
            remove_paths,
        });

        let current_paths = self
            .current_item()
            .map(|item| item.paths.clone())
            .unwrap_or_default();
        self.rescan_preserving_position(&current_paths);
        self.persist_session_best_effort();
        Ok(count)
    }

    /// Re-scans the open folder and keeps the queue on `focus_paths` when it
    /// still exists, otherwise on the closest position.
    fn rescan_preserving_position(&mut self, focus_paths: &[String]) {
        let Some(folder) = self.folder_path.clone() else {
            return;
        };
        let previous_index = self.current_index;
        let Ok(mut items) = scan_folder(&folder, self.scan_recursive) else {
            return;
        };
        prepare_sorted_items(&mut items, self.sort_mode);
        self.items = items;

        if let Some(idx) = self.find_item_index_by_paths(focus_paths, &self.items) {
            self.current_index = idx;
        } else if self.items.is_empty() {
            self.current_index = 0;
        } else {
            self.current_index = previous_index.min(self.items.len() - 1);
        }
        self.session_complete = self.items.is_empty();
        self.mark_subfolders_dirty();
    }

    pub fn batch_presets(&self) -> Vec<crate::batch::BatchPreset> {
        self.app_settings.batch_presets.clone()
    }

    pub fn save_batch_preset(
        &mut self,
        preset: crate::batch::BatchPreset,
    ) -> Result<Vec<crate::batch::BatchPreset>, String> {
        if let Some(existing) = self
            .app_settings
            .batch_presets
            .iter_mut()
            .find(|p| p.id == preset.id)
        {
            *existing = preset;
        } else {
            self.app_settings.batch_presets.push(preset);
        }
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        Ok(self.app_settings.batch_presets.clone())
    }

    pub fn delete_batch_preset(
        &mut self,
        id: &str,
    ) -> Result<Vec<crate::batch::BatchPreset>, String> {
        self.app_settings.batch_presets.retain(|p| p.id != id);
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        Ok(self.app_settings.batch_presets.clone())
    }

    /// Remembers the last used batch settings. Destructive output modes are
    /// never remembered as such: they come back as "write to a subfolder" so
    /// the confirmation dialog has to be answered again.
    pub fn remember_batch_settings(
        &mut self,
        settings: &crate::batch::BatchSettings,
    ) -> Result<(), String> {
        let mut stored = settings.clone();
        if matches!(stored.output, crate::batch::OutputMode::ReplaceOriginal { .. }) {
            stored.output = crate::batch::OutputMode::default();
        }
        self.app_settings.last_batch_settings = Some(stored);
        save_app_settings(&self.app_data_dir, &self.app_settings)
    }

    pub fn set_armed_folder(&mut self, folder: Option<String>) -> Result<(), String> {
        if let Some(ref rel) = folder {
            let root = self.folder_path.clone().ok_or("No folder open")?;
            let validated = validate_rel_folder(&root, rel)?;
            self.armed_folder = Some(validated);
        } else {
            self.armed_folder = None;
        }
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn toggle_favorite(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim().replace('\\', "/");
        if let Some(pos) = self
            .app_settings
            .favorite_folders
            .iter()
            .position(|f| f == &folder)
        {
            self.app_settings.favorite_folders.remove(pos);
        } else {
            self.app_settings.favorite_folders.push(folder);
        }
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        Ok(())
    }

    pub fn set_options(
        &mut self,
        sort_mode: SortMode,
        scan_recursive: bool,
        rename_mode: RenameMode,
    ) -> Result<(), String> {
        let current_paths = self
            .current_item()
            .map(|item| item.paths.clone())
            .unwrap_or_default();
        let recursive_changed = self.scan_recursive != scan_recursive;
        let sort_changed = self.sort_mode != sort_mode;

        self.sort_mode = sort_mode;
        self.scan_recursive = scan_recursive;
        self.rename_mode = rename_mode;

        if let Some(folder) = self.folder_path.clone() {
            if recursive_changed {
                self.items = scan_folder(&folder, self.scan_recursive)?;
                prepare_sorted_items(&mut self.items, self.sort_mode);
                self.subfolders_dirty = true;
            } else if sort_changed {
                if self.sort_mode == SortMode::ExifDate {
                    crate::media::enrich_exif_dates_for_sort(&mut self.items);
                }
                sort_items(&mut self.items, self.sort_mode);
            }

            if !current_paths.is_empty() {
                self.current_index = self
                    .find_item_index_by_paths(&current_paths, &self.items)
                    .unwrap_or_else(|| {
                        self.current_index
                            .min(self.items.len().saturating_sub(1))
                    });
            } else {
                self.current_index = self
                    .current_index
                    .min(self.items.len().saturating_sub(1));
            }
        }
        self.persist_session_best_effort();
        Ok(())
    }

    pub fn set_locale(&mut self, locale: String) -> Result<(), String> {
        self.app_settings.locale = locale;
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        Ok(())
    }

    pub fn set_ui_preferences(
        &mut self,
        layout_mode: LayoutMode,
        show_metadata: bool,
        video_with_sound: bool,
    ) -> Result<AppSettings, String> {
        self.app_settings.layout_mode = layout_mode;
        self.app_settings.show_metadata = show_metadata;
        self.app_settings.video_with_sound = video_with_sound;
        save_app_settings(&self.app_data_dir, &self.app_settings)?;
        Ok(self.app_settings.clone())
    }

    pub fn complete_first_run(&mut self) -> Result<(), String> {
        self.app_settings.first_run_completed = true;
        save_app_settings(&self.app_data_dir, &self.app_settings)
    }

    fn remember_folder(&mut self, folder: &str) {
        self.recent_folders.retain(|f| f != folder);
        self.recent_folders.insert(0, folder.to_string());
        self.recent_folders.truncate(12);
    }

    fn remove_current_from_queue(&mut self) -> Result<(), String> {
        let index = self.current_index;
        if index < self.items.len() {
            self.items.remove(index);
        }

        if self.items.is_empty() {
            self.current_index = 0;
            self.session_complete = true;
        } else {
            self.advance_after_processing(index, false);
        }

        self.persist_session_best_effort();
        Ok(())
    }

    fn refresh_after_rename(
        &mut self,
        old_index: usize,
        new_paths: &[String],
    ) -> Result<(), String> {
        if self.items.is_empty() {
            self.current_index = 0;
            self.session_complete = true;
            self.persist_session_best_effort();
            return Ok(());
        }

        let updated_id = self.update_item_after_rename(old_index, new_paths)?;
        let advance_from = if self.sort_mode == SortMode::FileName {
            sort_items(&mut self.items, self.sort_mode);
            self.items
                .iter()
                .position(|item| item.id == updated_id)
                .unwrap_or(old_index)
        } else {
            old_index
        };

        self.advance_after_processing(advance_from, true);
        self.persist_session_best_effort();
        Ok(())
    }

    fn update_item_after_rename(
        &mut self,
        index: usize,
        new_paths: &[String],
    ) -> Result<String, String> {
        let item = self
            .items
            .get_mut(index)
            .ok_or("Renamed item missing from queue")?;
        let primary = PathBuf::from(&new_paths[0]);

        item.paths = new_paths.to_vec();
        item.id = new_paths[0].clone();
        item.file_name = primary
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        item.extension = primary
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        item.is_video = crate::media::is_video_extension(&item.extension);
        refresh_item_size(item);

        Ok(item.id.clone())
    }

    fn insert_item_sorted(&mut self, item: MediaItem) {
        self.items.push(item);
        sort_items(&mut self.items, self.sort_mode);
    }

    fn apply_undo_to_queue(
        &mut self,
        action: &UndoAction,
        focus_paths: &[String],
        moves: &[PathPair],
    ) {
        match action {
            UndoAction::Rename { .. } => {
                let post_rename_paths: Vec<String> = moves.iter().map(|pair| pair.from.clone()).collect();
                if let Some(idx) = self.find_item_index_by_paths(&post_rename_paths, &self.items) {
                    let _ = self.update_item_after_rename(idx, focus_paths);
                    self.current_index = if self.sort_mode == SortMode::FileName {
                        sort_items(&mut self.items, self.sort_mode);
                        self.find_item_index_by_paths(focus_paths, &self.items)
                            .unwrap_or(idx)
                    } else {
                        idx
                    };
                } else if let Some(idx) = self.find_item_index_by_paths(focus_paths, &self.items) {
                    self.current_index = idx;
                }
            }
            UndoAction::Trash { .. } | UndoAction::MoveToFolder { .. } => {
                let item = build_media_item_from_paths(focus_paths);
                self.insert_item_sorted(item);
                if let Some(idx) = self.find_item_index_by_paths(focus_paths, &self.items) {
                    self.current_index = idx;
                }
                if matches!(action, UndoAction::MoveToFolder { .. }) {
                    self.mark_subfolders_dirty();
                }
            }
            UndoAction::FlattenToRoot { .. } => {
                if let Some(folder) = self.folder_path.clone() {
                    self.scan_recursive = true;
                    self.items = scan_folder(&folder, true).unwrap_or_default();
                    prepare_sorted_items(&mut self.items, self.sort_mode);
                    if let Some(idx) = self.find_item_index_by_paths(focus_paths, &self.items) {
                        self.current_index = idx;
                    } else if self.items.is_empty() {
                        self.current_index = 0;
                    } else {
                        self.current_index = self
                            .current_index
                            .min(self.items.len().saturating_sub(1));
                    }
                    self.mark_subfolders_dirty();
                }
            }
            UndoAction::ConvertMedia { .. } => {
                self.rescan_preserving_position(focus_paths);
            }
            UndoAction::TrimVideo { .. } => {
                if let Some(idx) = self.find_item_index_by_paths(focus_paths, &self.items) {
                    self.current_index = idx;
                    if let Some(item) = self.items.get_mut(idx) {
                        refresh_item_size(item);
                    }
                }
            }
        }

        if self.items.is_empty() {
            self.current_index = 0;
        } else {
            self.current_index = self
                .current_index
                .min(self.items.len().saturating_sub(1));
        }
    }

    fn refresh_subfolders_cache(&mut self) {
        self.cached_subfolders = self
            .folder_path
            .as_ref()
            .map(|path| list_subfolders(path))
            .unwrap_or_default();
        self.subfolders_dirty = false;
    }

    fn mark_subfolders_dirty(&mut self) {
        self.subfolders_dirty = true;
    }

    fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        if self.undo_stack.len() > MAX_UNDO {
            let overflow = self.undo_stack.len() - MAX_UNDO;
            self.undo_stack.drain(0..overflow);
            self.undo_overflow_notified = true;
        }
    }

    fn sanitize_inflated_session_stats(&mut self) {
        let flatten_moves: u32 = self
            .undo_stack
            .iter()
            .filter_map(|action| {
                if let UndoAction::FlattenToRoot { moves, .. } = action {
                    Some(moves.len() as u32)
                } else {
                    None
                }
            })
            .sum();

        if flatten_moves > 0 && self.stats.moved >= flatten_moves {
            self.stats.moved = self.stats.moved.saturating_sub(flatten_moves);
        }

        let queue_len = self.items.len();
        let removed = self.stats.moved as usize + self.stats.trashed as usize;
        if queue_len == 0 || removed == 0 {
            return;
        }

        let apparent_total = queue_len + removed;
        if apparent_total <= queue_len + queue_len / 20 || self.stats.moved as usize <= queue_len / 2 {
            return;
        }

        let Some(folder) = self.folder_path.clone() else {
            return;
        };
        let Ok(recursive_items) = scan_folder(&folder, true) else {
            return;
        };
        let album_size = recursive_items.len();
        if album_size == 0 || apparent_total <= album_size + album_size / 50 {
            return;
        }

        let inflation = apparent_total.saturating_sub(album_size);
        self.stats.moved = self
            .stats
            .moved
            .saturating_sub(inflation.min(self.stats.moved as usize) as u32);
    }

    fn revert_stat(&mut self, kind: UndoStatKind) {
        match kind {
            UndoStatKind::Renamed => {
                self.stats.renamed = self.stats.renamed.saturating_sub(1);
            }
            UndoStatKind::Trashed => {
                self.stats.trashed = self.stats.trashed.saturating_sub(1);
            }
            UndoStatKind::Moved => {
                self.stats.moved = self.stats.moved.saturating_sub(1);
            }
            UndoStatKind::None => {}
        }
    }

    fn persist_session(&self) -> Result<(), String> {
        let Some(folder) = &self.folder_path else {
            return Ok(());
        };

        let current_item_paths = self
            .current_item()
            .map(|item| item.paths.clone())
            .unwrap_or_default();

        let session = SessionData {
            folder_path: folder.to_string_lossy().to_string(),
            current_index: self.current_index,
            current_item_paths,
            sort_mode: self.sort_mode,
            scan_recursive: self.scan_recursive,
            rename_mode: self.rename_mode,
            counter_map: self.counter_map.clone(),
            recent_folders: self.recent_folders.clone(),
            armed_folder: self.armed_folder.clone(),
            undo_stack: self.undo_stack.clone(),
            stats: self.stats.clone(),
            processed_paths: self.processed_paths.iter().cloned().collect(),
        };

        save_session(folder, &session)
    }

    fn persist_session_best_effort(&self) {
        let _ = self.persist_session();
    }

    fn ok(&mut self, message: &str) -> ActionResult {
        let mut message = message.to_string();
        if self.undo_overflow_notified {
            message = format!("{message} (Older undo history was trimmed.)");
            self.undo_overflow_notified = false;
        }
        ActionResult {
            success: true,
            message,
            state: self.to_frontend_state(),
        }
    }

    fn fail(&mut self, message: &str) -> ActionResult {
        ActionResult {
            success: false,
            message: message.to_string(),
            state: self.to_frontend_state(),
        }
    }
}

fn session_index_fallback(saved_paths: &[String], items: &[MediaItem]) -> Option<usize> {
    items.iter().position(|item| {
        saved_paths
            .iter()
            .any(|path| item.paths.iter().any(|p| p == path))
    })
}

pub type SharedState = Mutex<AppState>;
