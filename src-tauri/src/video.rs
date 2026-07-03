use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use crate::models::{VideoPreviewInfo, VideoPreviewMode};

const WEB_NATIVE_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];

pub struct FfmpegTools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl FfmpegTools {
    pub fn locate() -> Result<Self, String> {
        let ffmpeg = find_binary("ffmpeg")?;
        let ffprobe = find_binary("ffprobe")?;
        Ok(Self { ffmpeg, ffprobe })
    }

    pub fn probe_duration(&self, path: &Path) -> Result<f64, String> {
        let output = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(path)
            .output()
            .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "ffprobe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        text.trim()
            .parse::<f64>()
            .map_err(|_| "Could not parse video duration.".into())
    }

    pub fn trim_lossless(
        &self,
        input: &Path,
        output: &Path,
        start_secs: f64,
        end_secs: f64,
    ) -> Result<(), String> {
        if end_secs <= start_secs + 0.05 {
            return Err("Trim range is too short.".into());
        }

        let output = Command::new(&self.ffmpeg)
            .arg("-y")
            .arg("-i")
            .arg(input)
            .arg("-ss")
            .arg(format!("{start_secs:.3}"))
            .arg("-to")
            .arg(format!("{end_secs:.3}"))
            .arg("-c")
            .arg("copy")
            .arg("-map")
            .arg("0")
            .arg("-avoid_negative_ts")
            .arg("make_zero")
            .arg(output)
            .output()
            .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

        if output.status.success() {
            return Ok(());
        }

        Err(format!(
            "ffmpeg trim failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }

    pub fn remux_for_web_preview(&self, input: &Path, output: &Path) -> Result<(), String> {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let output_cmd = Command::new(&self.ffmpeg)
            .arg("-y")
            .arg("-i")
            .arg(input)
            .arg("-c")
            .arg("copy")
            .arg("-movflags")
            .arg("+faststart")
            .arg(output)
            .output()
            .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

        if output_cmd.status.success() {
            return Ok(());
        }

        Err(format!(
            "ffmpeg remux failed: {}",
            String::from_utf8_lossy(&output_cmd.stderr)
        ))
    }

    pub fn capture_poster_frame(&self, input: &Path, output: &Path) -> Result<(), String> {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let output_cmd = Command::new(&self.ffmpeg)
            .arg("-y")
            .arg("-ss")
            .arg("0.5")
            .arg("-i")
            .arg(input)
            .arg("-frames:v")
            .arg("1")
            .arg("-q:v")
            .arg("3")
            .arg(output)
            .output()
            .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

        if output_cmd.status.success() {
            return Ok(());
        }

        Err(format!(
            "ffmpeg poster failed: {}",
            String::from_utf8_lossy(&output_cmd.stderr)
        ))
    }
}

fn find_binary(name: &str) -> Result<PathBuf, String> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            name.to_string(),
            format!("/opt/homebrew/bin/{name}"),
            format!("/usr/local/bin/{name}"),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            name.to_string(),
            format!("{name}.exe"),
            format!(r"C:\ffmpeg\bin\{name}.exe"),
        ]
    } else {
        vec![name.to_string(), format!("/usr/bin/{name}")]
    };

    for candidate in candidates {
        if Command::new(&candidate)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(PathBuf::from(candidate));
        }
    }

    Err(format!(
        "FFmpeg not found ({name}). Install it to trim videos losslessly (e.g. brew install ffmpeg)."
    ))
}

pub fn trim_backup_path(folder: &Path, video_path: &Path) -> PathBuf {
    let session = folder
        .join(crate::path_util::APP_FOLDER_NAME)
        .join("trim-backups");
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let ext = video_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f");
    session.join(format!("{stem}_{stamp}.{ext}"))
}

pub fn resolve_video_preview(source: &Path, cache_dir: &Path) -> VideoPreviewInfo {
    let source_string = source.to_string_lossy().to_string();
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if WEB_NATIVE_EXTENSIONS.contains(&extension.as_str()) {
        return VideoPreviewInfo {
            playback_path: source_string,
            poster_path: None,
            preview_mode: VideoPreviewMode::Native,
            hint: None,
        };
    }

    let cache_key = preview_cache_key(source);
    let proxy_path = cache_dir.join(format!("{cache_key}.mp4"));
    let poster_path = cache_dir.join(format!("{cache_key}.jpg"));

    if proxy_path.is_file() {
        return VideoPreviewInfo {
            playback_path: proxy_path.to_string_lossy().to_string(),
            poster_path: poster_path.is_file().then(|| poster_path.to_string_lossy().to_string()),
            preview_mode: VideoPreviewMode::Proxy,
            hint: None,
        };
    }

    let Ok(tools) = FfmpegTools::locate() else {
        return VideoPreviewInfo {
            playback_path: source_string,
            poster_path: None,
            preview_mode: VideoPreviewMode::Unavailable,
            hint: None,
        };
    };

    if tools.remux_for_web_preview(source, &proxy_path).is_ok() {
        let _ = tools.capture_poster_frame(source, &poster_path);
        return VideoPreviewInfo {
            playback_path: proxy_path.to_string_lossy().to_string(),
            poster_path: poster_path.is_file().then(|| poster_path.to_string_lossy().to_string()),
            preview_mode: VideoPreviewMode::Proxy,
            hint: None,
        };
    }

    let poster = maybe_poster(&tools, source, &poster_path);
    VideoPreviewInfo {
        playback_path: source_string,
        poster_path: poster,
        preview_mode: VideoPreviewMode::Unavailable,
        hint: None,
    }
}

fn maybe_poster(tools: &FfmpegTools, source: &Path, poster_path: &Path) -> Option<String> {
    tools.capture_poster_frame(source, poster_path).ok()?;
    poster_path.is_file().then(|| poster_path.to_string_lossy().to_string())
}

fn preview_cache_key(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(&mut hasher);
        if let Ok(modified) = meta.modified() {
            modified
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}
