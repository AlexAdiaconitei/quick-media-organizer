//! Pure helpers that turn `BatchSettings` into ffmpeg arguments and output
//! paths. Everything here is side-effect free so it can be unit tested without
//! ffmpeg installed.

use std::path::{Path, PathBuf};

#[cfg(test)]
use super::{AudioMode, VideoSettings};
use super::{
    BatchMediaType, BatchSettings, ConflictPolicy, ImageFormat, ImageSettings, VideoCodec,
};
use crate::media::{is_media_extension, is_video_extension};

/// Image extensions ffmpeg can decode but (in most builds) not encode.
const DECODE_ONLY_IMAGE_EXTENSIONS: &[&str] = &["heic", "heif"];

pub fn lower_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub fn classify_path(path: &Path) -> Option<BatchMediaType> {
    let ext = lower_extension(path);
    if ext.is_empty() || !is_media_extension(&ext) {
        return None;
    }
    if is_video_extension(&ext) {
        Some(BatchMediaType::Video)
    } else {
        Some(BatchMediaType::Image)
    }
}

/// Extension of the converted file, without the dot.
pub fn output_extension(
    media: BatchMediaType,
    settings: &BatchSettings,
    source: &Path,
) -> Result<String, String> {
    let source_ext = lower_extension(source);
    match media {
        BatchMediaType::Video => Ok(match settings.video.codec {
            // Stream copy keeps whatever container the source used, except for
            // containers that cannot hold the copied streams unchanged.
            VideoCodec::Copy if matches!(source_ext.as_str(), "mkv" | "avi") => source_ext,
            _ => "mp4".to_string(),
        }),
        BatchMediaType::Image => match settings.image.format {
            ImageFormat::Jpeg => Ok("jpg".to_string()),
            ImageFormat::Webp => Ok("webp".to_string()),
            ImageFormat::Avif => Ok("avif".to_string()),
            ImageFormat::Png => Ok("png".to_string()),
            ImageFormat::Keep => {
                if DECODE_ONLY_IMAGE_EXTENSIONS.contains(&source_ext.as_str()) {
                    return Err(format!(
                        "Cannot write .{source_ext} back out — pick a target format (JPEG/WebP/AVIF/PNG)."
                    ));
                }
                Ok(if source_ext == "jpeg" {
                    "jpg".to_string()
                } else {
                    source_ext
                })
            }
        },
    }
}

/// ffmpeg arguments between the input and the output file.
#[cfg(test)]
pub fn video_flags(settings: &VideoSettings) -> Vec<String> {
    super::video_backend::software_video_flags(settings)
}

pub fn image_flags(settings: &ImageSettings, out_ext: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec!["-frames:v".into(), "1".into()];

    args.push("-map_metadata".into());
    args.push(if settings.keep_metadata { "0" } else { "-1" }.into());

    match out_ext {
        "jpg" | "jpeg" => {
            args.extend(["-c:v", "mjpeg"].map(String::from));
            args.push("-q:v".into());
            args.push(jpeg_qscale(settings.quality).to_string());
            args.extend(["-pix_fmt", "yuvj420p"].map(String::from));
        }
        "webp" => {
            args.extend(["-c:v", "libwebp"].map(String::from));
            args.push("-quality".into());
            args.push(settings.quality.to_string());
            args.extend(["-compression_level", "6"].map(String::from));
        }
        "avif" => {
            args.extend(["-c:v", "libaom-av1"].map(String::from));
            args.push("-crf".into());
            args.push(avif_crf(settings.quality).to_string());
            args.extend(["-still-picture", "1", "-cpu-used", "6"].map(String::from));
        }
        "png" => {
            args.extend(["-c:v", "png", "-compression_level", "9"].map(String::from));
        }
        "gif" | "bmp" | "tif" | "tiff" => {
            // Keep-format fallbacks: no quality knob, just re-encode/resize.
        }
        other => {
            return Err(format!("Unsupported image output format: .{other}"));
        }
    }

    if let Some(max_edge) = settings.max_edge {
        args.push("-vf".into());
        args.push(image_scale_filter(max_edge));
    }

    Ok(args)
}

/// Clamp the long edge, keep the aspect ratio, never upscale.
pub fn image_scale_filter(max_edge: u32) -> String {
    format!("scale='if(gt(iw,ih),min({max_edge},iw),-2)':'if(gt(iw,ih),-2,min({max_edge},ih))'")
}

/// UI quality (1 worst … 100 best) mapped to ffmpeg's mjpeg qscale (2 best … 31 worst).
pub fn jpeg_qscale(quality: u8) -> u32 {
    let quality = quality.clamp(1, 100) as f32;
    let scale = 31.0 - (quality - 1.0) * (29.0 / 99.0);
    scale.round().clamp(2.0, 31.0) as u32
}

/// UI quality mapped to libaom's CRF (0 best … 63 worst).
pub fn avif_crf(quality: u8) -> u32 {
    let quality = quality.clamp(1, 100) as f32;
    let crf = 63.0 - (quality * 63.0 / 100.0);
    crf.round().clamp(0.0, 63.0) as u32
}

/// Final name for a converted file, before conflict resolution.
pub fn output_file_name(source: &Path, ext: &str, suffix: Option<&str>) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("media");
    let suffix = suffix
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    format!("{stem}{suffix}.{ext}")
}

/// Where the converted file must land. `exists` is injected so this stays pure
/// (and testable without touching the filesystem).
///
/// Returns `Ok(None)` when the policy says to skip an existing destination.
/// Overwriting the source file is never allowed here — that is what
/// `OutputMode::ReplaceOriginal` is for, and it goes through a temp file.
pub fn resolve_output_path(
    source: &Path,
    dest_dir: &Path,
    ext: &str,
    suffix: Option<&str>,
    policy: ConflictPolicy,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<Option<PathBuf>, String> {
    let candidate = dest_dir.join(output_file_name(source, ext, suffix));
    let clashes_with_source = candidate == source;

    if !clashes_with_source && !exists(&candidate) {
        return Ok(Some(candidate));
    }

    if !clashes_with_source && policy == ConflictPolicy::Overwrite {
        return Ok(Some(candidate));
    }

    if !clashes_with_source && policy == ConflictPolicy::Skip {
        return Ok(None);
    }

    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("media");
    let suffix = suffix
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    for n in 2..1000 {
        let candidate = dest_dir.join(format!("{stem}{suffix} ({n}).{ext}"));
        if candidate != source && !exists(&candidate) {
            return Ok(Some(candidate));
        }
    }

    Err(format!(
        "Could not find a free name for {} in {}.",
        source.display(),
        dest_dir.display()
    ))
}

/// Hidden temp file written by ffmpeg; renamed into place only after the
/// output has been verified.
pub fn temp_output_path(final_path: &Path, index: usize) -> PathBuf {
    let ext = final_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");
    let dir = final_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!(".qmo-tmp-{index}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_files(_: &Path) -> bool {
        false
    }

    #[test]
    fn classifies_media_by_extension() {
        assert_eq!(
            classify_path(Path::new("/a/IMG_1.MOV")),
            Some(BatchMediaType::Video)
        );
        assert_eq!(
            classify_path(Path::new("/a/IMG_1.HEIC")),
            Some(BatchMediaType::Image)
        );
        assert_eq!(classify_path(Path::new("/a/notes.txt")), None);
        assert_eq!(classify_path(Path::new("/a/noext")), None);
    }

    #[test]
    fn h265_flags_carry_crf_preset_and_apple_tag() {
        let settings = VideoSettings {
            codec: VideoCodec::H265,
            crf: 28,
            speed_preset: "medium".into(),
            max_height: Some(1080),
            ..VideoSettings::default()
        };
        let flags = video_flags(&settings).join(" ");
        assert!(flags.contains("-c:v libx265"));
        assert!(flags.contains("-crf 28"));
        assert!(flags.contains("-preset medium"));
        assert!(flags.contains("-tag:v hvc1"));
        assert!(flags.contains("scale=-2:'min(1080,ih)'"));
        assert!(flags.contains("-c:a aac -b:a 128k"));
        assert!(flags.contains("-movflags +faststart"));
    }

    #[test]
    fn no_scale_filter_without_height_limit() {
        let settings = VideoSettings {
            max_height: None,
            ..VideoSettings::default()
        };
        let flags = video_flags(&settings).join(" ");
        assert!(!flags.contains("-vf"));
    }

    #[test]
    fn copy_codec_only_remuxes() {
        let settings = VideoSettings {
            codec: VideoCodec::Copy,
            ..VideoSettings::default()
        };
        let flags = video_flags(&settings);
        assert_eq!(
            flags,
            vec!["-map", "0", "-c", "copy", "-movflags", "+faststart"]
        );
    }

    #[test]
    fn dropping_audio_never_maps_an_audio_stream() {
        let settings = VideoSettings {
            audio: AudioMode::Drop,
            ..VideoSettings::default()
        };
        let flags = video_flags(&settings).join(" ");
        assert!(!flags.contains("0:a?"));
        assert!(flags.contains("-an"));
    }

    #[test]
    fn av1_uses_numeric_preset() {
        let settings = VideoSettings {
            codec: VideoCodec::Av1,
            speed_preset: "slow".into(),
            ..VideoSettings::default()
        };
        let flags = video_flags(&settings).join(" ");
        assert!(flags.contains("-c:v libsvtav1"));
        assert!(flags.contains("-preset 4"));
    }

    #[test]
    fn stripping_metadata_flips_map_metadata() {
        let settings = VideoSettings {
            keep_metadata: false,
            ..VideoSettings::default()
        };
        assert!(video_flags(&settings)
            .join(" ")
            .contains("-map_metadata -1"));
    }

    #[test]
    fn quality_maps_per_format() {
        assert_eq!(jpeg_qscale(100), 2);
        assert_eq!(jpeg_qscale(1), 31);
        assert!(jpeg_qscale(85) < jpeg_qscale(50));
        assert_eq!(avif_crf(100), 0);
        assert!(avif_crf(80) < avif_crf(40));
    }

    #[test]
    fn image_flags_match_target_format() {
        let settings = ImageSettings {
            quality: 80,
            max_edge: Some(2560),
            ..ImageSettings::default()
        };
        let webp = image_flags(&settings, "webp").unwrap().join(" ");
        assert!(webp.contains("-c:v libwebp"));
        assert!(webp.contains("-quality 80"));
        assert!(webp.contains("scale='if(gt(iw,ih),min(2560,iw),-2)'"));

        let jpeg = image_flags(&settings, "jpg").unwrap().join(" ");
        assert!(jpeg.contains("-c:v mjpeg"));
        assert!(jpeg.contains("-q:v"));

        assert!(image_flags(&settings, "heic").is_err());
    }

    #[test]
    fn output_extension_follows_target_format() {
        let mut settings = BatchSettings::default();
        settings.image.format = ImageFormat::Webp;
        assert_eq!(
            output_extension(BatchMediaType::Image, &settings, Path::new("a/x.heic")).unwrap(),
            "webp"
        );

        settings.image.format = ImageFormat::Keep;
        assert_eq!(
            output_extension(BatchMediaType::Image, &settings, Path::new("a/x.JPEG")).unwrap(),
            "jpg"
        );
        assert!(output_extension(BatchMediaType::Image, &settings, Path::new("a/x.heic")).is_err());

        assert_eq!(
            output_extension(BatchMediaType::Video, &settings, Path::new("a/x.avi")).unwrap(),
            "mp4"
        );
        settings.video.codec = VideoCodec::Copy;
        assert_eq!(
            output_extension(BatchMediaType::Video, &settings, Path::new("a/x.mkv")).unwrap(),
            "mkv"
        );
    }

    #[test]
    fn output_name_applies_suffix() {
        assert_eq!(
            output_file_name(Path::new("/a/IMG_1.MOV"), "mp4", Some("-opt")),
            "IMG_1-opt.mp4"
        );
        assert_eq!(
            output_file_name(Path::new("/a/IMG_1.MOV"), "mp4", Some("  ")),
            "IMG_1.mp4"
        );
    }

    #[test]
    fn conflicts_follow_the_policy() {
        let dir = PathBuf::from("/out");
        let source = PathBuf::from("/in/IMG_1.mov");
        let taken = |p: &Path| p == Path::new("/out/IMG_1.mp4");

        assert_eq!(
            resolve_output_path(&source, &dir, "mp4", None, ConflictPolicy::Skip, &taken).unwrap(),
            None
        );
        assert_eq!(
            resolve_output_path(
                &source,
                &dir,
                "mp4",
                None,
                ConflictPolicy::Overwrite,
                &taken
            )
            .unwrap(),
            Some(PathBuf::from("/out/IMG_1.mp4"))
        );
        assert_eq!(
            resolve_output_path(&source, &dir, "mp4", None, ConflictPolicy::Rename, &taken)
                .unwrap(),
            Some(PathBuf::from("/out/IMG_1 (2).mp4"))
        );
        assert_eq!(
            resolve_output_path(
                &source,
                &dir,
                "mp4",
                None,
                ConflictPolicy::Rename,
                &no_files
            )
            .unwrap(),
            Some(PathBuf::from("/out/IMG_1.mp4"))
        );
    }

    #[test]
    fn never_targets_the_source_file_even_when_overwriting() {
        let dir = PathBuf::from("/in");
        let source = PathBuf::from("/in/IMG_1.mp4");
        let resolved = resolve_output_path(
            &source,
            &dir,
            "mp4",
            None,
            ConflictPolicy::Overwrite,
            &no_files,
        )
        .unwrap()
        .unwrap();
        assert_ne!(resolved, source);
        assert_eq!(resolved, PathBuf::from("/in/IMG_1 (2).mp4"));
    }

    #[test]
    fn temp_files_are_hidden_and_keep_the_extension() {
        let temp = temp_output_path(Path::new("/out/IMG_1.mp4"), 7);
        assert_eq!(temp, PathBuf::from("/out/.qmo-tmp-7.mp4"));
    }
}
