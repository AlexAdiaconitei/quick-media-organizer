//! End-to-end checks against a real ffmpeg binary.
//!
//! Unit tests elsewhere use a fake encoder, so they prove the runner's logic
//! but not that the argument strings are accepted by ffmpeg. These do, and
//! they skip themselves when ffmpeg is not installed so CI stays green.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::runner::{
    new_job_status, plan_items, run_job, BatchEvents, FfmpegEncoder, MediaEncoder,
};
use super::{
    BatchItemState, BatchItemStatus, BatchJobStatus, BatchProgressSummary, BatchSettings,
    ImageFormat, OutputMode, VideoCodec,
};
use crate::video::{find_binary, no_window_command};

struct SilentEvents;

impl BatchEvents for SilentEvents {
    fn item(&self, _item: &BatchItemStatus) {}
    fn progress(&self, _summary: &BatchProgressSummary) {}
    fn done(&self, _job: &BatchJobStatus) {}
}

fn ffmpeg_available() -> bool {
    FfmpegEncoder::locate().is_ok()
}

/// Resolved the same way the app does, so the fixtures work even when ffmpeg
/// is only reachable through an install folder and not through PATH.
fn ffmpeg_bin() -> PathBuf {
    find_binary("ffmpeg").expect("ffmpeg should be resolvable")
}

fn ffprobe_bin() -> PathBuf {
    find_binary("ffprobe").expect("ffprobe should be resolvable")
}

/// Renders a short synthetic clip with audio.
fn make_video(path: &Path) {
    let status = no_window_command(&ffmpeg_bin())
        .args([
            "-hide_banner",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=1280x720:rate=30:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=2",
            "-c:v",
            "libx264",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg should render the fixture");
    assert!(status.success(), "fixture video was not created");
}

fn make_image(path: &Path) {
    let status = no_window_command(&ffmpeg_bin())
        .args([
            "-hide_banner",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=1600x900:duration=1",
            "-frames:v",
            "1",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg should render the fixture");
    assert!(status.success(), "fixture image was not created");
}

fn dimensions(path: &Path) -> (u32, u32) {
    let output = no_window_command(&ffprobe_bin())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
        ])
        .arg(path)
        .output()
        .expect("ffprobe should read the file");
    let text = String::from_utf8_lossy(&output.stdout);
    let (width, height) = text
        .trim()
        .split_once('x')
        .unwrap_or_else(|| panic!("unexpected ffprobe output: {text:?}"));
    (width.parse().unwrap(), height.parse().unwrap())
}

fn run(paths: Vec<String>, settings: BatchSettings) -> BatchJobStatus {
    let plans = plan_items(&paths, &settings);
    let statuses: Vec<BatchItemStatus> = plans.iter().map(|p| p.status.clone()).collect();
    let job = Arc::new(Mutex::new(new_job_status(
        "smoke".into(),
        statuses,
        None,
        false,
    )));
    run_job(
        Arc::clone(&job),
        plans,
        settings,
        Arc::new(FfmpegEncoder::locate().unwrap()),
        Arc::new(SilentEvents),
        Arc::new(AtomicBool::new(false)),
    );
    let status = job.lock().unwrap().clone();
    status
}

fn lenient(output: OutputMode) -> BatchSettings {
    BatchSettings {
        output,
        // Synthetic footage compresses unpredictably; the size rules are
        // covered by the unit tests.
        skip_if_larger: false,
        skip_if_savings_below_pct: None,
        concurrency: 1,
        ..BatchSettings::default()
    }
}

fn out_dir(dir: &Path) -> OutputMode {
    OutputMode::CustomFolder {
        path: dir.join("out").to_string_lossy().to_string(),
    }
}

#[test]
fn ffmpeg_accepts_the_video_flags_and_downscales() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clip.mp4");
    make_video(&source);

    let mut settings = lenient(out_dir(dir.path()));
    settings.video.codec = VideoCodec::H265;
    settings.video.crf = 30;
    settings.video.speed_preset = "fast".into();
    settings.video.max_height = Some(480);

    let job = run(vec![source.to_string_lossy().to_string()], settings);

    assert_eq!(
        job.failed, 0,
        "ffmpeg rejected the flags: {:?}",
        job.items[0].error
    );
    assert_eq!(job.items[0].state, BatchItemState::Done);

    let output = PathBuf::from(job.items[0].output_path.clone().unwrap());
    assert!(output.is_file());
    // The quoted scale expression is the fragile part: prove it really applied.
    assert_eq!(dimensions(&output), (854, 480));
    let encoder = FfmpegEncoder::locate().unwrap();
    let duration = encoder.probe_duration(&output).unwrap();
    assert!((duration - 2.0).abs() < 0.4, "unexpected duration {duration}");
}

#[test]
fn ffmpeg_accepts_stream_copy_remux() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clip.mp4");
    make_video(&source);

    let mut settings = lenient(out_dir(dir.path()));
    settings.video.codec = VideoCodec::Copy;

    let job = run(vec![source.to_string_lossy().to_string()], settings);

    assert_eq!(job.failed, 0, "remux failed: {:?}", job.items[0].error);
    assert_eq!(dimensions(&PathBuf::from(
        job.items[0].output_path.clone().unwrap()
    )), (1280, 720));
}

#[test]
fn ffmpeg_accepts_the_image_flags_and_resizes_the_long_edge() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("photo.png");
    make_image(&source);

    for (format, expected_ext) in [
        (ImageFormat::Jpeg, "jpg"),
        (ImageFormat::Webp, "webp"),
    ] {
        let mut settings = lenient(OutputMode::CustomFolder {
            path: dir.path().join(expected_ext).to_string_lossy().to_string(),
        });
        settings.image.format = format;
        settings.image.quality = 80;
        settings.image.max_edge = Some(800);

        let job = run(vec![source.to_string_lossy().to_string()], settings);

        assert_eq!(
            job.failed, 0,
            "ffmpeg rejected the {expected_ext} flags: {:?}",
            job.items[0].error
        );
        let output = PathBuf::from(job.items[0].output_path.clone().unwrap());
        assert_eq!(output.extension().unwrap(), expected_ext);
        assert_eq!(dimensions(&output), (800, 450));
    }
}

#[test]
fn a_corrupt_source_fails_without_leaving_an_output() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("broken.mp4");
    std::fs::write(&source, b"not a video at all").unwrap();

    let job = run(
        vec![source.to_string_lossy().to_string()],
        lenient(out_dir(dir.path())),
    );

    assert_eq!(job.failed, 1);
    assert_eq!(job.done, 0);
    let produced: Vec<_> = std::fs::read_dir(dir.path().join("out"))
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(produced.is_empty(), "a failed encode left files behind");
}

/// Guards the check that decides whether the UI warns about HEIC.
#[test]
fn capabilities_report_this_build() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let tools = crate::video::FfmpegTools::locate().unwrap();
    let capabilities = tools.capabilities();
    assert!(capabilities.available);
    assert!(capabilities.version.is_some());
    // Every mainstream build ships x264; if this fails the parsing is broken.
    assert!(capabilities.h264);
}
