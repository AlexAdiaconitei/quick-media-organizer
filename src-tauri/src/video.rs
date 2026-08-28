use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use crate::batch::video_backend::{detect_video_backends, VideoEncodeAttempt};
use crate::batch::{FfmpegCapabilities, VideoBackendCapability};
use crate::models::{VideoPreviewInfo, VideoPreviewMode};

const WEB_NATIVE_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];
static VIDEO_BACKENDS: OnceLock<Vec<VideoBackendCapability>> = OnceLock::new();

pub struct EncodeRequest<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub input_flags: &'a [String],
    pub output_flags: &'a [String],
    pub total_duration: Option<f64>,
    pub cancel: &'a AtomicBool,
    pub on_progress: &'a dyn Fn(f32),
}

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
    pub fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String> {
        if let Some(parent) = request.output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut command = no_window_command(&self.ffmpeg);
        command
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-y")
            .args(request.input_flags)
            .arg("-i")
            .arg(request.input)
            .args(request.output_flags)
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg(request.output)
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

        // Reading stdout directly would block until ffmpeg emits its next
        // progress line. A stalled decoder would then ignore cancellation.
        // Forward lines through a channel so this thread can poll the flag and
        // kill the child within one interval even when stdout is silent.
        let (progress_tx, progress_rx) = mpsc::channel();
        let stdout = child.stdout.take();
        let stdout_handle = std::thread::spawn(move || {
            if let Some(stdout) = stdout {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if progress_tx.send(line).is_err() {
                        break;
                    }
                }
            }
        });

        let mut killed = false;
        let mut stdout_disconnected = false;
        let status = loop {
            if request.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                killed = true;
                break child
                    .wait()
                    .map_err(|e| format!("Failed to wait for cancelled ffmpeg: {e}"))?;
            }

            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Failed to poll ffmpeg: {e}"))?
            {
                break status;
            }

            if stdout_disconnected {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            match progress_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    if let Some(progress) = parse_progress_line(&line, request.total_duration) {
                        (request.on_progress)(progress);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => stdout_disconnected = true,
            }
        };

        let _ = stdout_handle.join();
        let stderr_tail = stderr_handle.join().unwrap_or_default();

        if killed {
            return Err(CANCELLED.to_string());
        }

        if status.success() {
            (request.on_progress)(1.0);
            return Ok(());
        }

        Err(format!("ffmpeg failed: {}", stderr_tail.trim()))
    }

    /// Which encoders/decoders this ffmpeg build actually ships, so the UI can
    /// hide codecs that would fail and warn about HEIC before starting a job.
    pub fn capabilities(&self) -> FfmpegCapabilities {
        let encoders = self
            .run_text(&["-hide_banner", "-encoders"])
            .unwrap_or_default();
        let decoders = self
            .run_text(&["-hide_banner", "-decoders"])
            .unwrap_or_default();
        let demuxers = self
            .run_text(&["-hide_banner", "-demuxers"])
            .unwrap_or_default();
        let version = self
            .run_text(&["-hide_banner", "-version"])
            .and_then(|text| text.lines().next().map(|l| l.trim().to_string()));
        let video_backends = VIDEO_BACKENDS
            .get_or_init(|| {
                detect_video_backends(&encoders, &|attempt| {
                    self.probe_video_encoder(attempt, Duration::from_secs(3))
                })
            })
            .clone();

        FfmpegCapabilities {
            available: true,
            h264: encoders.contains("libx264"),
            h265: encoders.contains("libx265"),
            av1: encoders.contains("libsvtav1"),
            webp: encoders.contains("libwebp"),
            avif: encoders.contains("libaom-av1"),
            // HEIC/HEIF files are ISOBMFF: ffmpeg reads them through the mov
            // demuxer plus the hevc decoder, not through a "heif" demuxer.
            heic_decode: decoders.contains("hevc") && demuxers.contains("mov,mp4"),
            video_backends,
            version,
        }
    }

    fn probe_video_encoder(
        &self,
        attempt: &VideoEncodeAttempt,
        timeout: Duration,
    ) -> Result<(), String> {
        let mut command = no_window_command(&self.ffmpeg);
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .args(&attempt.input_flags)
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=64x64:r=25:d=0.08")
            .args(&attempt.output_flags)
            .arg("-frames:v")
            .arg("1")
            .arg("-f")
            .arg("null")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("Failed to probe hardware encoder: {error}"))?;
        let started = Instant::now();
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("Failed to poll hardware probe: {error}"))?
            {
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return if status.success() {
                    Ok(())
                } else {
                    Err(stderr.trim().to_string())
                };
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Hardware encoder probe timed out.".into());
            }
            std::thread::sleep(Duration::from_millis(25));
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
    let value = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    let micros: f64 = value.trim().parse().ok()?;
    // out_time_ms is actually microseconds in ffmpeg's progress output.
    let seconds = micros / 1_000_000.0;
    Some(((seconds / total) as f32).clamp(0.0, 1.0))
}

/// Spawns console-less on Windows; a plain `Command` everywhere else.
pub fn no_window_command(program: &Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

/// Resolved binaries, so repeated lookups do not spawn a probe process each
/// time. Only successes are cached: a user who installs ffmpeg while the app
/// is open should be picked up on the next try.
static RESOLVED: Mutex<Option<Vec<(String, PathBuf)>>> = Mutex::new(None);

fn cached_binary(name: &str) -> Option<PathBuf> {
    let guard = RESOLVED.lock().ok()?;
    let entries = guard.as_ref()?;
    entries
        .iter()
        .find(|(cached, _)| cached == name)
        .map(|(_, path)| path.clone())
}

fn cache_binary(name: &str, path: &Path) {
    if let Ok(mut guard) = RESOLVED.lock() {
        let entries = guard.get_or_insert_with(Vec::new);
        entries.retain(|(cached, _)| cached != name);
        entries.push((name.to_string(), path.to_path_buf()));
    }
}

pub(crate) fn find_binary(name: &str) -> Result<PathBuf, String> {
    if let Some(path) = cached_binary(name) {
        return Ok(path);
    }

    for candidate in binary_candidates(name) {
        if no_window_command(&candidate)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            cache_binary(name, &candidate);
            return Ok(candidate);
        }
    }

    Err(missing_ffmpeg_message(name))
}

/// PATH first, then the usual install locations. Windows needs the extra work:
/// installers add their folder to the *user* PATH in the registry, which
/// running processes never pick up until they are restarted.
fn binary_candidates(name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // A bundled copy sits next to the executable (Tauri's externalBin), and it
    // wins over whatever the machine happens to have installed: it is the
    // build we tested against.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(bundled_binary_name(name)));
        }
    }

    candidates.push(PathBuf::from(name));

    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from(format!("/opt/homebrew/bin/{name}")));
        candidates.push(PathBuf::from(format!("/usr/local/bin/{name}")));
    } else if cfg!(target_os = "windows") {
        candidates.push(PathBuf::from(format!("{name}.exe")));

        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            // winget shim folder.
            candidates.push(
                local
                    .join("Microsoft")
                    .join("WinGet")
                    .join("Links")
                    .join(format!("{name}.exe")),
            );
            // winget keeps the real binaries in a versioned folder, e.g.
            // Packages/Gyan.FFmpeg_.../ffmpeg-9.0-full_build/bin/ffmpeg.exe
            candidates.extend(winget_package_binaries(&local, name));
        }

        candidates.push(PathBuf::from(format!(r"C:\ffmpeg\bin\{name}.exe")));
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\ffmpeg\bin\{name}.exe"
        )));

        if let Some(program_data) = std::env::var_os("ProgramData").map(PathBuf::from) {
            candidates.push(
                program_data
                    .join("chocolatey")
                    .join("bin")
                    .join(format!("{name}.exe")),
            );
        }
        if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            candidates.push(home.join("scoop").join("shims").join(format!("{name}.exe")));
        }
    } else {
        candidates.push(PathBuf::from(format!("/usr/bin/{name}")));
        candidates.push(PathBuf::from(format!("/usr/local/bin/{name}")));
        candidates.push(PathBuf::from(format!("/snap/bin/{name}")));
    }

    candidates
}

fn bundled_binary_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Every `…/WinGet/Packages/<pkg>/<build>/bin/<name>.exe` that exists.
fn winget_package_binaries(local_app_data: &Path, name: &str) -> Vec<PathBuf> {
    let packages = local_app_data
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    let Ok(entries) = fs::read_dir(&packages) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for package in entries.filter_map(Result::ok) {
        if !package.path().is_dir() {
            continue;
        }
        let Ok(builds) = fs::read_dir(package.path()) else {
            continue;
        };
        for build in builds.filter_map(Result::ok) {
            let candidate = build.path().join("bin").join(format!("{name}.exe"));
            if candidate.is_file() {
                found.push(candidate);
            }
        }
    }
    found
}

fn missing_ffmpeg_message(name: &str) -> String {
    let hint = if cfg!(target_os = "windows") {
        "install it with `winget install Gyan.FFmpeg` and reopen the app"
    } else if cfg!(target_os = "macos") {
        "install it with `brew install ffmpeg`"
    } else {
        "install it with your package manager, e.g. `sudo apt install ffmpeg`"
    };
    format!("FFmpeg not found ({name}). To trim and convert videos, {hint}.")
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
        // Touched on use so eviction drops the least recently *used* proxy,
        // not merely the oldest one built.
        let _ = filetime::set_file_mtime(&proxy_path, filetime::FileTime::now());
        return VideoPreviewInfo {
            playback_path: proxy_path.to_string_lossy().to_string(),
            poster_path: poster_path
                .is_file()
                .then(|| poster_path.to_string_lossy().to_string()),
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
        prune_preview_cache(cache_dir, PREVIEW_CACHE_MAX_BYTES);
        return VideoPreviewInfo {
            playback_path: proxy_path.to_string_lossy().to_string(),
            poster_path: poster_path
                .is_file()
                .then(|| poster_path.to_string_lossy().to_string()),
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

/// Proxies are disposable — rebuilding one costs a single remux — so the cache
/// is capped and the least recently used files are dropped. Without this it
/// grows by one copy of every non-MP4 video the user ever looked at.
const PREVIEW_CACHE_MAX_BYTES: u64 = 1_500_000_000;

fn prune_preview_cache(cache_dir: &Path, max_bytes: u64) {
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };

    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((
                meta.modified().unwrap_or(UNIX_EPOCH),
                meta.len(),
                entry.path(),
            ))
        })
        .collect();

    let mut total: u64 = files.iter().map(|(_, len, _)| *len).sum();
    if total <= max_bytes {
        return;
    }

    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in files {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

fn maybe_poster(tools: &FfmpegTools, source: &Path, poster_path: &Path) -> Option<String> {
    tools.capture_poster_frame(source, poster_path).ok()?;
    poster_path
        .is_file()
        .then(|| poster_path.to_string_lossy().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundled_binary_wins_over_the_system_one() {
        let candidates = binary_candidates("ffmpeg");
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
            .expect("the test binary has a parent directory");

        assert_eq!(
            candidates[0].parent(),
            Some(exe_dir.as_path()),
            "the copy shipped in the installer must be probed first"
        );
        assert_eq!(
            candidates[1],
            PathBuf::from("ffmpeg"),
            "then whatever is on PATH"
        );
    }

    #[test]
    fn progress_lines_are_parsed_against_the_duration() {
        assert_eq!(
            parse_progress_line("out_time_us=5000000", Some(10.0)),
            Some(0.5)
        );
        assert_eq!(
            parse_progress_line("out_time_us=20000000", Some(10.0)),
            Some(1.0)
        );
        assert_eq!(parse_progress_line("frame=12", Some(10.0)), None);
        assert_eq!(parse_progress_line("out_time_us=5000000", None), None);
    }
}
