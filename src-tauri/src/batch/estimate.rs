//! Fast preflight size estimate for a batch selection.
//!
//! The module samples a few files through the same encoder interface as the
//! real runner. Video samples encode at most three seconds. The measured ratio
//! is then applied to the selected bytes of the same media type.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::ffmpeg_args::{classify_path, image_flags, output_extension};
use super::runner::MediaEncoder;
use super::video_backend::{next_attempt_index, resolve_video_encoding, VideoEncodingPlan};
use super::{BatchEstimate, BatchMediaType, BatchSettings, FfmpegCapabilities};
use crate::video::{EncodeRequest, CANCELLED};

const VIDEO_SAMPLE_SECONDS: f64 = 3.0;
const MAX_VIDEO_SAMPLES: usize = 3;
const MAX_IMAGE_SAMPLES: usize = 5;
static SAMPLE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct MediaEstimate {
    total_bytes: u64,
    sampled_source_bytes: u64,
    sampled_output_bytes: f64,
    sampled_files: usize,
    failed_samples: usize,
}

impl MediaEstimate {
    fn estimated_bytes(&self) -> u64 {
        if self.total_bytes == 0 {
            return 0;
        }
        if self.sampled_source_bytes == 0 {
            return self.total_bytes;
        }
        let ratio = self.sampled_output_bytes / self.sampled_source_bytes as f64;
        (self.total_bytes as f64 * ratio).round().max(0.0) as u64
    }
}

pub fn estimate_batch<E: MediaEncoder>(
    paths: &[String],
    settings: &BatchSettings,
    capabilities: &FfmpegCapabilities,
    encoder: &E,
    temp_dir: &Path,
    cancel: &AtomicBool,
) -> Result<BatchEstimate, String> {
    fs::create_dir_all(temp_dir)
        .map_err(|error| format!("Could not create estimate folder: {error}"))?;
    let video_plan = resolve_video_encoding(&settings.video, &capabilities.video_backends)?;
    let mut videos = MediaEstimate::default();
    let mut images = MediaEstimate::default();
    let mut total_files = 0;

    for raw in paths {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let path = Path::new(raw);
        let Some(media_type) = classify_path(path) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(path) else {
            continue;
        };
        total_files += 1;
        let target = match media_type {
            BatchMediaType::Video => &mut videos,
            BatchMediaType::Image => &mut images,
        };
        target.total_bytes = target.total_bytes.saturating_add(metadata.len());

        let limit = match media_type {
            BatchMediaType::Video => MAX_VIDEO_SAMPLES,
            BatchMediaType::Image => MAX_IMAGE_SAMPLES,
        };
        if target.sampled_files + target.failed_samples >= limit {
            continue;
        }

        let sample = match media_type {
            BatchMediaType::Video => {
                sample_video(path, settings, &video_plan, encoder, temp_dir, cancel)
            }
            BatchMediaType::Image => sample_image(path, settings, encoder, temp_dir, cancel),
        };
        match sample {
            Ok(estimated_full_size) => {
                target.sampled_source_bytes =
                    target.sampled_source_bytes.saturating_add(metadata.len());
                target.sampled_output_bytes += estimated_full_size;
                target.sampled_files += 1;
            }
            Err(error) if error == CANCELLED => return Err(error),
            Err(_) => target.failed_samples += 1,
        }
    }

    let _ = fs::remove_dir(temp_dir);
    Ok(BatchEstimate {
        bytes_before: videos.total_bytes.saturating_add(images.total_bytes),
        estimated_bytes_after: videos
            .estimated_bytes()
            .saturating_add(images.estimated_bytes()),
        sampled_files: videos.sampled_files + images.sampled_files,
        total_files,
        failed_samples: videos.failed_samples + images.failed_samples,
    })
}

fn sample_video<E: MediaEncoder>(
    input: &Path,
    settings: &BatchSettings,
    plan: &VideoEncodingPlan,
    encoder: &E,
    temp_dir: &Path,
    cancel: &AtomicBool,
) -> Result<f64, String> {
    let source_size = fs::metadata(input)
        .map_err(|error| error.to_string())?
        .len();
    if settings.video.codec == super::VideoCodec::Copy {
        return Ok(source_size as f64);
    }
    let duration = encoder
        .probe_duration(input)
        .ok_or_else(|| "Could not read sample duration.".to_string())?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err("Sample duration is invalid.".into());
    }
    let sample_duration = duration.clamp(0.05, VIDEO_SAMPLE_SECONDS);
    let extension = output_extension(BatchMediaType::Video, settings, input)?;
    let output = sample_path(temp_dir, &extension);
    let mut attempted = HashSet::new();
    let mut current = 0usize;

    let result = loop {
        let attempt = plan
            .attempts
            .get(current)
            .ok_or_else(|| "No estimate encoder attempt was configured.".to_string())?;
        let mut output_flags = attempt.output_flags.clone();
        output_flags.extend(["-t".into(), format!("{sample_duration:.3}")]);
        let encoded = encoder.encode(EncodeRequest {
            input,
            output: &output,
            input_flags: &attempt.input_flags,
            output_flags: &output_flags,
            total_duration: Some(sample_duration),
            cancel,
            on_progress: &|_| {},
        });
        match encoded {
            Ok(()) => break Ok(()),
            Err(error) => {
                attempted.insert(current);
                let Some(next) = next_attempt_index(plan, current, &error, &attempted) else {
                    break Err(error);
                };
                let _ = fs::remove_file(&output);
                current = next;
            }
        }
    };
    let measured = result.and_then(|()| {
        fs::metadata(&output)
            .map(|metadata| metadata.len())
            .map_err(|error| error.to_string())
    });
    let _ = fs::remove_file(&output);
    measured.map(|bytes| bytes as f64 * duration / sample_duration)
}

fn sample_image<E: MediaEncoder>(
    input: &Path,
    settings: &BatchSettings,
    encoder: &E,
    temp_dir: &Path,
    cancel: &AtomicBool,
) -> Result<f64, String> {
    let extension = output_extension(BatchMediaType::Image, settings, input)?;
    let output = sample_path(temp_dir, &extension);
    let flags = image_flags(&settings.image, &extension)?;
    let result = encoder.encode(EncodeRequest {
        input,
        output: &output,
        input_flags: &[],
        output_flags: &flags,
        total_duration: None,
        cancel,
        on_progress: &|_| {},
    });
    if result.is_ok() && settings.image.keep_metadata {
        crate::metadata::copy_exif(input, &output)?;
    }
    let measured = result.and_then(|()| {
        fs::metadata(&output)
            .map(|metadata| metadata.len())
            .map_err(|error| error.to_string())
    });
    let _ = fs::remove_file(&output);
    measured.map(|bytes| bytes as f64)
}

fn sample_path(temp_dir: &Path, extension: &str) -> PathBuf {
    let id = SAMPLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    temp_dir.join(format!("estimate-{}-{id}.{extension}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    use crate::batch::runner::MediaEncoder;
    use crate::batch::{VideoBackend, VideoBackendCapability, VideoCodec};

    struct RatioEncoder;

    impl MediaEncoder for RatioEncoder {
        fn probe_duration(&self, _path: &Path) -> Option<f64> {
            Some(10.0)
        }

        fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String> {
            let is_video = request.output_flags.windows(2).any(|pair| pair[0] == "-t");
            let bytes = if is_video { 15 } else { 20 };
            fs::write(request.output, vec![b'x'; bytes]).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn extrapolates_three_second_video_and_full_image_samples() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("clip.mov");
        let image = dir.path().join("photo.jpg");
        fs::write(&video, vec![b'v'; 100]).unwrap();
        fs::write(&image, vec![b'i'; 100]).unwrap();
        let settings = BatchSettings::default();
        let capabilities = FfmpegCapabilities {
            available: true,
            video_backends: vec![VideoBackendCapability {
                backend: VideoBackend::Software,
                codecs: vec![VideoCodec::H265],
                available: true,
                reason: None,
            }],
            ..FfmpegCapabilities::default()
        };

        let estimate = estimate_batch(
            &[
                video.to_string_lossy().to_string(),
                image.to_string_lossy().to_string(),
            ],
            &settings,
            &capabilities,
            &RatioEncoder,
            &dir.path().join("samples"),
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(estimate.bytes_before, 200);
        assert_eq!(estimate.estimated_bytes_after, 70);
        assert_eq!(estimate.sampled_files, 2);
        assert_eq!(estimate.failed_samples, 0);
    }
}
