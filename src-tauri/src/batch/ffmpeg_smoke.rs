//! End-to-end checks against a real ffmpeg binary.
//!
//! Unit tests elsewhere use a fake encoder, so they prove the runner's logic
//! but not that the argument strings are accepted by ffmpeg. These do, and
//! they skip themselves when ffmpeg is not installed so CI stays green.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::estimate::estimate_batch;
use super::runner::{
    new_job_status, plan_items, plan_items_with_capabilities, run_job, BatchEvents, FfmpegEncoder,
    MediaEncoder,
};
use super::{
    BatchItemState, BatchItemStatus, BatchJobStatus, BatchProgressSummary, BatchSettings,
    HardwareAcceleration, ImageFormat, OutputMode, VideoBackend, VideoCodec,
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
            "-g",
            "15",
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
    run_with_cancel(paths, settings, Arc::new(AtomicBool::new(false)))
}

fn run_with_cancel(
    paths: Vec<String>,
    settings: BatchSettings,
    cancel: Arc<AtomicBool>,
) -> BatchJobStatus {
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
        cancel,
    );
    let status = job.lock().unwrap().clone();
    status
}

fn hardware_preference(backend: VideoBackend) -> HardwareAcceleration {
    match backend {
        VideoBackend::Software => HardwareAcceleration::Software,
        VideoBackend::Nvidia => HardwareAcceleration::Nvidia,
        VideoBackend::Intel => HardwareAcceleration::Intel,
        VideoBackend::Amd => HardwareAcceleration::Amd,
        VideoBackend::VideoToolbox => HardwareAcceleration::VideoToolbox,
        VideoBackend::Vaapi => HardwareAcceleration::Vaapi,
    }
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
    assert!(
        (duration - 2.0).abs() < 0.4,
        "unexpected duration {duration}"
    );
}

#[test]
fn mixed_mp4_mov_and_avi_sources_complete_in_one_job() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let mp4 = dir.path().join("camera-a.mp4");
    let mov = dir.path().join("camera-b.mov");
    let avi = dir.path().join("camera-c.avi");
    make_video(&mp4);

    let mov_status = no_window_command(&ffmpeg_bin())
        .args(["-hide_banner", "-y", "-i"])
        .arg(&mp4)
        .args(["-c", "copy"])
        .arg(&mov)
        .status()
        .unwrap();
    assert!(mov_status.success(), "MOV fixture was not created");

    let avi_status = no_window_command(&ffmpeg_bin())
        .args(["-hide_banner", "-y", "-i"])
        .arg(&mp4)
        .args(["-c:v", "mpeg4", "-q:v", "5", "-c:a", "libmp3lame"])
        .arg(&avi)
        .status()
        .unwrap();
    assert!(avi_status.success(), "AVI fixture was not created");

    let mut settings = lenient(out_dir(dir.path()));
    settings.video.codec = VideoCodec::H264;
    settings.video.speed_preset = "fast".into();
    settings.video.hardware_acceleration = HardwareAcceleration::Software;
    let paths = [&mp4, &mov, &avi]
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    let job = run(paths, settings);

    assert_eq!(job.done, 3, "mixed job failed: {:?}", job.items);
    assert_eq!(job.failed, 0);
    for item in job.items {
        let output = PathBuf::from(item.output_path.unwrap());
        assert!(output.is_file());
        assert!(FfmpegEncoder::locate()
            .unwrap()
            .probe_duration(&output)
            .is_some());
    }
}

#[test]
fn estimate_encodes_a_real_video_sample() {
    let Ok(encoder) = FfmpegEncoder::locate() else {
        eprintln!("skipping: ffmpeg not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("estimate-source.mp4");
    make_video(&source);
    let settings = lenient(out_dir(dir.path()));
    let capabilities = encoder.capabilities();

    let estimate = estimate_batch(
        &[source.to_string_lossy().to_string()],
        &settings,
        &capabilities,
        &encoder,
        &dir.path().join("estimate-cache"),
        &AtomicBool::new(false),
    )
    .unwrap();

    assert_eq!(estimate.sampled_files, 1);
    assert_eq!(estimate.failed_samples, 0);
    assert!(estimate.estimated_bytes_after > 0);
}

#[test]
fn a_detected_hardware_backend_completes_a_real_encode() {
    let Ok(encoder) = FfmpegEncoder::locate() else {
        eprintln!("skipping: ffmpeg not installed");
        return;
    };
    let capabilities = encoder.capabilities();
    let Some((backend, codec)) = capabilities.video_backends.iter().find_map(|capability| {
        capability
            .available
            .then(|| capability.codecs.first().copied())
            .flatten()
            .map(|codec| (capability.backend, codec))
    }) else {
        eprintln!("skipping: no usable hardware video encoder detected");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("hardware-source.mp4");
    make_video(&source);
    let mut settings = lenient(out_dir(dir.path()));
    settings.video.codec = codec;
    settings.video.hardware_acceleration = hardware_preference(backend);
    settings.video.max_height = Some(480);

    let paths = vec![source.to_string_lossy().to_string()];
    let plans = plan_items_with_capabilities(&paths, &settings, &capabilities.video_backends);
    let statuses = plans.iter().map(|item| item.status.clone()).collect();
    let job = Arc::new(Mutex::new(new_job_status(
        "hardware-smoke".into(),
        statuses,
        None,
        false,
    )));
    run_job(
        Arc::clone(&job),
        plans,
        settings,
        Arc::new(encoder),
        Arc::new(SilentEvents),
        Arc::new(AtomicBool::new(false)),
    );
    let result = job.lock().unwrap().clone();

    assert_eq!(
        result.failed, 0,
        "hardware encode failed: {:?}",
        result.items[0].error
    );
    assert_eq!(result.items[0].state, BatchItemState::Done);
    assert_eq!(result.items[0].encoder_backend, Some(backend));
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
    assert_eq!(
        dimensions(&PathBuf::from(job.items[0].output_path.clone().unwrap())),
        (1280, 720)
    );
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

    for (format, expected_ext) in [(ImageFormat::Jpeg, "jpg"), (ImageFormat::Webp, "webp")] {
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

/// Optional bulk HEIC validation for release machines that provide a real
/// fixture through `QMO_HEIC_TEST_FILE`. CI without that fixture still covers
/// the capability gate and the image runner with generated formats.
#[test]
fn one_hundred_real_heic_files_convert_in_one_job_when_a_fixture_is_available() {
    let Some(fixture) = std::env::var_os("QMO_HEIC_TEST_FILE").map(PathBuf::from) else {
        eprintln!("skipping: QMO_HEIC_TEST_FILE is not set");
        return;
    };
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    assert!(fixture.is_file(), "HEIC fixture does not exist");

    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::with_capacity(100);
    for index in 0..100 {
        let path = dir.path().join(format!("photo-{index:03}.heic"));
        fs::copy(&fixture, &path).unwrap();
        paths.push(path.to_string_lossy().to_string());
    }
    let mut settings = lenient(out_dir(dir.path()));
    settings.image.format = ImageFormat::Jpeg;
    settings.image.keep_metadata = false;
    settings.concurrency = 4;

    let job = run(paths, settings);

    assert_eq!(job.done, 100, "HEIC failures: {:?}", job.items);
    assert_eq!(job.failed, 0);
    assert_eq!(fs::read_dir(dir.path().join("out")).unwrap().count(), 100);
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

#[test]
fn cancelling_a_real_encode_kills_ffmpeg_and_removes_the_temporary_file() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cancel-source.mp4");
    make_video(&source);
    let original = fs::read(&source).unwrap();
    let mut settings = lenient(out_dir(dir.path()));
    settings.video.codec = VideoCodec::H265;
    settings.video.speed_preset = "slow".into();
    settings.video.hardware_acceleration = HardwareAcceleration::Software;
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancel);
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(40));
        signal.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let job = run_with_cancel(vec![source.to_string_lossy().to_string()], settings, cancel);
    canceller.join().unwrap();

    assert!(job.cancelled);
    assert_eq!(fs::read(&source).unwrap(), original);
    let output_files = fs::read_dir(dir.path().join("out"))
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(output_files, 0, "cancel left a temporary or final output");
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

/// The whole trim path against real ffmpeg: the file the app keeps showing
/// must be the trimmed one, the original must survive in the backup folder,
/// and undo must put it back.
#[test]
fn trimming_replaces_the_file_and_undo_restores_it() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("clip.mp4");
    make_video(&clip);

    let encoder = FfmpegEncoder::locate().unwrap();
    let before = encoder.probe_duration(&clip).unwrap();
    let bytes_before = fs::metadata(&clip).unwrap().len();

    let mut state = crate::state::AppState::new(dir.path().join("appdata"));
    state.open_folder(dir.path().to_path_buf()).unwrap();

    let result = state.trim_current_video(0.5, 1.5).unwrap();
    assert!(result.success, "{}", result.message_key);

    let after = encoder
        .probe_duration(&clip)
        .expect("the trimmed file is still a readable video");
    assert!(
        after < before - 0.4,
        "expected a shorter clip, got {after} from {before}"
    );

    // The queue keeps pointing at the same path, with the new size.
    let item = result.state.item.expect("an item stays selected");
    assert_eq!(item.paths[0], clip.to_string_lossy());
    assert_eq!(item.size_bytes, fs::metadata(&clip).unwrap().len());
    assert_ne!(item.size_bytes, bytes_before, "the preview key must change");

    let backups: Vec<_> = fs::read_dir(
        dir.path()
            .join(crate::path_util::APP_FOLDER_NAME)
            .join("trim-backups"),
    )
    .unwrap()
    .filter_map(Result::ok)
    .collect();
    assert_eq!(backups.len(), 1, "the original is kept once");

    let undone = state.undo_last().unwrap();
    assert!(undone.success, "{}", undone.message_key);
    let restored = encoder.probe_duration(&clip).unwrap();
    assert!(
        (restored - before).abs() < 0.3,
        "undo should restore the full clip, got {restored} from {before}"
    );
}
