use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use crate::batch::FfmpegCapabilities;
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
        let output = no_window_command(&self.ffprobe)
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

        let output = no_window_command(&self.ffmpeg)
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

        let output_cmd = no_window_command(&self.ffmpeg)
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

        let output_cmd = no_window_command(&self.ffmpeg)
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

    /// Runs ffmpeg to completion, streaming progress from `-progress pipe:1`.
    ///
    /// `flags` are the arguments between input and output (see
    /// `crate::batch::ffmpeg_args`). Returns [`CANCELLED`] as the error message
    /// when `cancel` is raised, so callers can tell it apart from a real
    /// failure.
    pub fn encode(
        &self,
        input: &Path,
        output: &Path,
        flags: &[String],
        total_duration: Option<f64>,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(f32),
    ) -> Result<(), String> {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut command = no_window_command(&self.ffmpeg);
        command
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-y")
            .arg("-i")
            .arg(input)
            .args(flags)
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

        // ffmpeg writes diagnostics to stderr; drain it on its own thread so a
        // full pipe buffer cannot deadlock the progress reader.
        let stderr = child.stderr.take();
        let stderr_handle = std::thread::spawn(move || {
            let mut tail = String::new();
            if let Some(stderr) = stderr {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    tail.push_str(&line);
                    tail.push('\n');
                    if tail.len() > 8000 {
                        let cut = tail.len() - 4000;
                        tail = tail.split_off(cut);
                    }
                }
            }
            tail
        });

        let mut killed = false;
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if cancel.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    killed = true;
                    break;
                }
                if let Some(progress) = parse_progress_line(&line, total_duration) {
                    on_progress(progress);
                }
            }
        }

        if !killed && cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            killed = true;
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for ffmpeg: {e}"))?;
        let stderr_tail = stderr_handle.join().unwrap_or_default();

        if killed {
            return Err(CANCELLED.to_string());
        }

        if status.success() {
            on_progress(1.0);
            return Ok(());
        }

        Err(format!("ffmpeg failed: {}", stderr_tail.trim()))
    }

    /// Which encoders/decoders this ffmpeg build actually ships, so the UI can
    /// hide codecs that would fail and warn about HEIC before starting a job.
    pub fn capabilities(&self) -> FfmpegCapabilities {
        let encoders = self.run_text(&["-hide_banner", "-encoders"]).unwrap_or_default();
        let demuxers = self.run_text(&["-hide_banner", "-demuxers"]).unwrap_or_default();
        let version = self
            .run_text(&["-hide_banner", "-version"])
            .and_then(|text| text.lines().next().map(|l| l.trim().to_string()));

        FfmpegCapabilities {
            available: true,
            h264: encoders.contains("libx264"),
            h265: encoders.contains("libx265"),
            av1: encoders.contains("libsvtav1"),
            webp: encoders.contains("libwebp"),
            avif: encoders.contains("libaom-av1"),
            heic_decode: demuxers.contains("heif") || demuxers.contains("HEIF"),
            version,
        }
    }

    fn run_text(&self, args: &[&str]) -> Option<String> {
        let output = no_window_command(&self.ffmpeg).args(args).output().ok()?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Some(text)
    }
}

/// Error message used when a job was cancelled rather than failing.
pub const CANCELLED: &str = "__cancelled__";

/// Parses one `key=value` line of ffmpeg's `-progress` stream into 0..1.
fn parse_progress_line(line: &str, total_duration: Option<f64>) -> Option<f32> {
    let total = total_duration.filter(|d| *d > 0.0)?;
    let value = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms="))?;
    let micros: f64 = value.trim().parse().ok()?;
    // out_time_ms is actually microseconds in ffmpeg's progress output.
    let seconds = micros / 1_000_000.0;
    Some(((seconds / total) as f32).clamp(0.0, 1.0))
}

/// Spawns console-less on Windows; a plain `Command` everywhere else.
pub fn no_window_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
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
        if no_window_command(Path::new(&candidate))
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
