//! Hardware encoder discovery and video encoding plans.
//!
//! Callers provide video intent and receive ordered FFmpeg attempts. Vendor
//! options, quality mappings, device probes and fallback rules stay here.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{
    AudioMode, HardwareAcceleration, VideoBackend, VideoBackendCapability, VideoCodec,
    VideoSettings,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoEncodeAttempt {
    pub backend: VideoBackend,
    pub audio: AudioMode,
    pub input_flags: Vec<String>,
    pub output_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncodingPlan {
    pub resolved_backend: VideoBackend,
    pub attempts: Vec<VideoEncodeAttempt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeFailureKind {
    Cancelled,
    BackendUnavailable,
    AudioIncompatible,
    InvalidMedia,
    Other,
}

#[cfg(test)]
pub fn software_capability() -> VideoBackendCapability {
    VideoBackendCapability {
        backend: VideoBackend::Software,
        codecs: vec![VideoCodec::H264, VideoCodec::H265, VideoCodec::Av1],
        available: true,
        reason: None,
    }
}

#[cfg(test)]
pub fn software_video_flags(settings: &VideoSettings) -> Vec<String> {
    build_attempt(settings, VideoBackend::Software, settings.audio)
        .expect("software supports every non-copy video codec")
        .output_flags
}

/// Detects usable devices, not just encoders compiled into FFmpeg. The probe
/// performs one real frame encode for every supported backend and codec.
pub fn detect_video_backends(
    encoders: &str,
    probe: &dyn Fn(&VideoEncodeAttempt) -> Result<(), String>,
) -> Vec<VideoBackendCapability> {
    detect_video_backends_for(platform_backends(), encoders, probe)
}

fn detect_video_backends_for(
    backends: &[VideoBackend],
    encoders: &str,
    probe: &dyn Fn(&VideoEncodeAttempt) -> Result<(), String>,
) -> Vec<VideoBackendCapability> {
    backends
        .iter()
        .copied()
        .map(|backend| {
            let mut codecs = Vec::new();
            let mut failures = Vec::new();
            let mut compiled_any = false;

            for codec in [VideoCodec::H264, VideoCodec::H265, VideoCodec::Av1] {
                let Some(encoder) = encoder_name(backend, codec) else {
                    continue;
                };
                if !has_encoder(encoders, encoder) {
                    continue;
                }
                compiled_any = true;
                let attempt = probe_attempt(backend, codec);
                match probe(&attempt) {
                    Ok(()) => codecs.push(codec),
                    Err(error) => failures.push((codec, probe_reason(&error))),
                }
            }

            let available = !codecs.is_empty();
            let reason = if available {
                None
            } else if compiled_any {
                Some(describe_failures(&failures))
            } else {
                Some("Encoder not included in this FFmpeg build.".into())
            };

            VideoBackendCapability {
                backend,
                codecs,
                available,
                reason,
            }
        })
        .collect()
}

pub fn resolve_video_encoding(
    settings: &VideoSettings,
    capabilities: &[VideoBackendCapability],
) -> Result<VideoEncodingPlan, String> {
    if settings.codec == VideoCodec::Copy {
        return Ok(VideoEncodingPlan {
            resolved_backend: VideoBackend::Software,
            attempts: vec![build_attempt(
                settings,
                VideoBackend::Software,
                settings.audio,
            )?],
        });
    }

    let resolved = match settings.hardware_acceleration {
        HardwareAcceleration::Software => VideoBackend::Software,
        HardwareAcceleration::Auto => capabilities
            .iter()
            .find(|capability| capability.available && capability.codecs.contains(&settings.codec))
            .map(|capability| capability.backend)
            .unwrap_or(VideoBackend::Software),
        preference => {
            let requested = preference_backend(preference).expect("auto/software handled above");
            let usable = capabilities.iter().any(|capability| {
                capability.backend == requested
                    && capability.available
                    && capability.codecs.contains(&settings.codec)
            });
            if !usable {
                return Err(format!(
                    "{} is not available for {} on this computer.",
                    backend_label(requested),
                    codec_label(settings.codec)
                ));
            }
            requested
        }
    };

    let mut attempts = Vec::new();
    push_attempt(&mut attempts, settings, resolved, settings.audio)?;
    if settings.audio == AudioMode::Copy {
        push_attempt(&mut attempts, settings, resolved, AudioMode::Aac)?;
    }
    if resolved.is_hardware() {
        push_attempt(
            &mut attempts,
            settings,
            VideoBackend::Software,
            settings.audio,
        )?;
        if settings.audio == AudioMode::Copy {
            push_attempt(
                &mut attempts,
                settings,
                VideoBackend::Software,
                AudioMode::Aac,
            )?;
        }
    }

    Ok(VideoEncodingPlan {
        resolved_backend: resolved,
        attempts,
    })
}

/// Chooses a fallback without retrying corrupt media or cancelled jobs.
pub fn next_attempt_index(
    plan: &VideoEncodingPlan,
    current: usize,
    error: &str,
    attempted: &HashSet<usize>,
) -> Option<usize> {
    let current_attempt = plan.attempts.get(current)?;
    let kind = classify_encode_failure(error);
    if matches!(
        kind,
        EncodeFailureKind::Cancelled | EncodeFailureKind::InvalidMedia
    ) {
        return None;
    }

    let mut wanted = Vec::new();
    match kind {
        EncodeFailureKind::BackendUnavailable => {
            wanted.push((VideoBackend::Software, current_attempt.audio));
            if current_attempt.audio == AudioMode::Copy {
                wanted.push((VideoBackend::Software, AudioMode::Aac));
            }
        }
        EncodeFailureKind::AudioIncompatible => {
            if current_attempt.audio == AudioMode::Copy {
                wanted.push((current_attempt.backend, AudioMode::Aac));
                wanted.push((VideoBackend::Software, AudioMode::Aac));
            }
        }
        EncodeFailureKind::Other => {
            if current_attempt.backend.is_hardware() {
                wanted.push((VideoBackend::Software, current_attempt.audio));
                if current_attempt.audio == AudioMode::Copy {
                    wanted.push((VideoBackend::Software, AudioMode::Aac));
                }
            } else if current_attempt.audio == AudioMode::Copy {
                wanted.push((VideoBackend::Software, AudioMode::Aac));
            }
        }
        EncodeFailureKind::Cancelled | EncodeFailureKind::InvalidMedia => {}
    }

    wanted.into_iter().find_map(|(backend, audio)| {
        plan.attempts
            .iter()
            .enumerate()
            .find(|(index, attempt)| {
                !attempted.contains(index) && attempt.backend == backend && attempt.audio == audio
            })
            .map(|(index, _)| index)
    })
}

pub fn classify_encode_failure(error: &str) -> EncodeFailureKind {
    if error == crate::video::CANCELLED {
        return EncodeFailureKind::Cancelled;
    }
    let lower = error.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "invalid data found",
            "moov atom not found",
            "could not find codec parameters",
            "error while decoding",
            "invalid nal unit",
        ],
    ) {
        return EncodeFailureKind::InvalidMedia;
    }
    if contains_any(
        &lower,
        &[
            "no capable devices found",
            "cannot load nvcuda",
            "driver does not support",
            "failed to initialise vaapi",
            "failed to initialize vaapi",
            "device setup failed",
            "no device available",
            "mfx session",
            "amf failed",
            "videotoolbox session",
            "hardware device",
            "unsupported device",
        ],
    ) {
        return EncodeFailureKind::BackendUnavailable;
    }
    if contains_any(
        &lower,
        &[
            "could not find tag for codec",
            "not currently supported in container",
            "audio codec",
            "could not write header",
        ],
    ) {
        return EncodeFailureKind::AudioIncompatible;
    }
    EncodeFailureKind::Other
}

pub fn backend_label(backend: VideoBackend) -> &'static str {
    match backend {
        VideoBackend::Software => "CPU (software)",
        VideoBackend::Nvidia => "NVIDIA NVENC",
        VideoBackend::Intel => "Intel Quick Sync",
        VideoBackend::Amd => "AMD AMF",
        VideoBackend::VideoToolbox => "Apple VideoToolbox",
        VideoBackend::Vaapi => "Linux VAAPI",
    }
}

fn codec_label(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "H.264",
        VideoCodec::H265 => "H.265",
        VideoCodec::Av1 => "AV1",
        VideoCodec::Copy => "stream copy",
    }
}

fn platform_backends() -> &'static [VideoBackend] {
    #[cfg(target_os = "windows")]
    {
        &[VideoBackend::Nvidia, VideoBackend::Intel, VideoBackend::Amd]
    }
    #[cfg(target_os = "macos")]
    {
        &[VideoBackend::VideoToolbox]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        &[VideoBackend::Vaapi]
    }
}

fn preference_backend(preference: HardwareAcceleration) -> Option<VideoBackend> {
    match preference {
        HardwareAcceleration::Nvidia => Some(VideoBackend::Nvidia),
        HardwareAcceleration::Intel => Some(VideoBackend::Intel),
        HardwareAcceleration::Amd => Some(VideoBackend::Amd),
        HardwareAcceleration::VideoToolbox => Some(VideoBackend::VideoToolbox),
        HardwareAcceleration::Vaapi => Some(VideoBackend::Vaapi),
        HardwareAcceleration::Auto | HardwareAcceleration::Software => None,
    }
}

fn push_attempt(
    attempts: &mut Vec<VideoEncodeAttempt>,
    settings: &VideoSettings,
    backend: VideoBackend,
    audio: AudioMode,
) -> Result<(), String> {
    if attempts
        .iter()
        .any(|attempt| attempt.backend == backend && attempt.audio == audio)
    {
        return Ok(());
    }
    attempts.push(build_attempt(settings, backend, audio)?);
    Ok(())
}

fn build_attempt(
    settings: &VideoSettings,
    backend: VideoBackend,
    audio: AudioMode,
) -> Result<VideoEncodeAttempt, String> {
    let mut input_flags = Vec::new();
    let mut output_flags = Vec::new();

    if settings.codec == VideoCodec::Copy {
        output_flags.extend(["-map", "0", "-c", "copy"].map(String::from));
        if settings.faststart {
            output_flags.extend(["-movflags", "+faststart"].map(String::from));
        }
        return Ok(VideoEncodeAttempt {
            backend: VideoBackend::Software,
            audio,
            input_flags,
            output_flags,
        });
    }

    let encoder = encoder_name(backend, settings.codec).ok_or_else(|| {
        format!(
            "{} cannot encode {}.",
            backend_label(backend),
            codec_label(settings.codec)
        )
    })?;

    output_flags.extend(["-map", "0:v:0"].map(String::from));
    if audio != AudioMode::Drop {
        output_flags.extend(["-map", "0:a?"].map(String::from));
    }
    output_flags.push("-map_metadata".into());
    output_flags.push(if settings.keep_metadata { "0" } else { "-1" }.into());
    output_flags.extend(["-c:v", encoder].map(String::from));

    add_backend_quality(&mut output_flags, backend, settings);

    let mut filters = Vec::new();
    if let Some(max_height) = settings.max_height {
        filters.push(format!("scale=-2:'min({max_height},ih)'"));
    }
    match backend {
        VideoBackend::Software
        | VideoBackend::Nvidia
        | VideoBackend::Amd
        | VideoBackend::VideoToolbox => {
            output_flags.extend(["-pix_fmt", "yuv420p"].map(String::from));
        }
        VideoBackend::Intel => filters.push("format=nv12".into()),
        VideoBackend::Vaapi => {
            input_flags.extend(["-vaapi_device", "/dev/dri/renderD128"].map(String::from));
            filters.extend(["format=nv12".into(), "hwupload".into()]);
        }
    }
    if !filters.is_empty() {
        output_flags.push("-vf".into());
        output_flags.push(filters.join(","));
    }

    if let Some(fps) = settings.max_fps {
        output_flags.push("-r".into());
        output_flags.push(fps.to_string());
    }

    match audio {
        AudioMode::Copy => output_flags.extend(["-c:a", "copy"].map(String::from)),
        AudioMode::Aac => {
            output_flags.extend(["-c:a", "aac"].map(String::from));
            output_flags.push("-b:a".into());
            output_flags.push(format!("{}k", settings.audio_bitrate_kbps));
        }
        AudioMode::Drop => output_flags.push("-an".into()),
    }

    if settings.codec == VideoCodec::H265 {
        output_flags.extend(["-tag:v", "hvc1"].map(String::from));
    }
    if settings.faststart {
        output_flags.extend(["-movflags", "+faststart"].map(String::from));
    }

    Ok(VideoEncodeAttempt {
        backend,
        audio,
        input_flags,
        output_flags,
    })
}

fn probe_attempt(backend: VideoBackend, codec: VideoCodec) -> VideoEncodeAttempt {
    let mut settings = VideoSettings {
        codec,
        audio: AudioMode::Drop,
        max_height: None,
        max_fps: None,
        faststart: false,
        keep_metadata: false,
        ..VideoSettings::default()
    };
    settings.hardware_acceleration = match backend {
        VideoBackend::Software => HardwareAcceleration::Software,
        VideoBackend::Nvidia => HardwareAcceleration::Nvidia,
        VideoBackend::Intel => HardwareAcceleration::Intel,
        VideoBackend::Amd => HardwareAcceleration::Amd,
        VideoBackend::VideoToolbox => HardwareAcceleration::VideoToolbox,
        VideoBackend::Vaapi => HardwareAcceleration::Vaapi,
    };
    build_attempt(&settings, backend, AudioMode::Drop).expect("known backend and codec")
}

fn add_backend_quality(flags: &mut Vec<String>, backend: VideoBackend, settings: &VideoSettings) {
    let quality = settings.crf.clamp(1, 51).to_string();
    match backend {
        VideoBackend::Software => {
            flags.extend(["-crf", quality.as_str()].map(String::from));
            flags.push("-preset".into());
            flags.push(match settings.codec {
                VideoCodec::Av1 => svt_av1_preset(&settings.speed_preset).to_string(),
                _ => settings.speed_preset.clone(),
            });
        }
        VideoBackend::Nvidia => {
            flags.extend(["-rc", "vbr", "-cq", quality.as_str(), "-b:v", "0"].map(String::from));
            flags.extend(["-preset", nvenc_preset(&settings.speed_preset)].map(String::from));
            flags.extend(["-tune", "hq"].map(String::from));
        }
        VideoBackend::Intel => {
            flags.extend(["-global_quality", quality.as_str()].map(String::from));
            flags.push("-preset".into());
            flags.push(settings.speed_preset.clone());
        }
        VideoBackend::Amd => {
            flags
                .extend(["-rc", "qvbr", "-qvbr_quality_level", quality.as_str()].map(String::from));
            flags.extend(
                ["-quality", generic_hardware_preset(&settings.speed_preset)].map(String::from),
            );
        }
        VideoBackend::VideoToolbox => {
            flags.extend(
                [
                    "-q:v",
                    videotoolbox_quality(settings.crf).to_string().as_str(),
                ]
                .map(String::from),
            );
            flags.push("-realtime".into());
            flags.push(
                if settings.speed_preset == "fast" {
                    "1"
                } else {
                    "0"
                }
                .into(),
            );
        }
        VideoBackend::Vaapi => {
            flags.extend(["-qp", quality.as_str()].map(String::from));
        }
    }
}

fn encoder_name(backend: VideoBackend, codec: VideoCodec) -> Option<&'static str> {
    match (backend, codec) {
        (VideoBackend::Software, VideoCodec::H264) => Some("libx264"),
        (VideoBackend::Software, VideoCodec::H265) => Some("libx265"),
        (VideoBackend::Software, VideoCodec::Av1) => Some("libsvtav1"),
        (VideoBackend::Nvidia, VideoCodec::H264) => Some("h264_nvenc"),
        (VideoBackend::Nvidia, VideoCodec::H265) => Some("hevc_nvenc"),
        (VideoBackend::Nvidia, VideoCodec::Av1) => Some("av1_nvenc"),
        (VideoBackend::Intel, VideoCodec::H264) => Some("h264_qsv"),
        (VideoBackend::Intel, VideoCodec::H265) => Some("hevc_qsv"),
        (VideoBackend::Intel, VideoCodec::Av1) => Some("av1_qsv"),
        (VideoBackend::Amd, VideoCodec::H264) => Some("h264_amf"),
        (VideoBackend::Amd, VideoCodec::H265) => Some("hevc_amf"),
        (VideoBackend::Amd, VideoCodec::Av1) => Some("av1_amf"),
        (VideoBackend::VideoToolbox, VideoCodec::H264) => Some("h264_videotoolbox"),
        (VideoBackend::VideoToolbox, VideoCodec::H265) => Some("hevc_videotoolbox"),
        (VideoBackend::VideoToolbox, VideoCodec::Av1) => Some("av1_videotoolbox"),
        (VideoBackend::Vaapi, VideoCodec::H264) => Some("h264_vaapi"),
        (VideoBackend::Vaapi, VideoCodec::H265) => Some("hevc_vaapi"),
        (VideoBackend::Vaapi, VideoCodec::Av1) => Some("av1_vaapi"),
        (_, VideoCodec::Copy) => None,
    }
}

fn has_encoder(encoders: &str, encoder: &str) -> bool {
    encoders
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(encoder))
}

fn nvenc_preset(speed: &str) -> &'static str {
    match speed {
        "slow" | "slower" | "veryslow" => "p7",
        "fast" | "faster" | "veryfast" | "ultrafast" => "p1",
        _ => "p4",
    }
}

fn generic_hardware_preset(speed: &str) -> &'static str {
    match speed {
        "slow" | "slower" | "veryslow" => "quality",
        "fast" | "faster" | "veryfast" | "ultrafast" => "speed",
        _ => "balanced",
    }
}

fn videotoolbox_quality(crf: u8) -> u8 {
    let crf = crf.clamp(1, 51) as u16;
    (((51 - crf) * 100) / 50).clamp(1, 100) as u8
}

fn svt_av1_preset(speed: &str) -> u8 {
    match speed {
        "veryslow" | "slower" => 3,
        "slow" => 4,
        "medium" => 6,
        "fast" | "faster" => 8,
        "veryfast" | "ultrafast" => 10,
        _ => 6,
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

/// One driver problem usually fails every codec with the same message, so the
/// codec prefix is only worth printing when the failures actually differ.
fn describe_failures(failures: &[(VideoCodec, String)]) -> String {
    let distinct: HashSet<&str> = failures.iter().map(|(_, why)| why.as_str()).collect();
    if distinct.len() == 1 {
        return failures[0].1.clone();
    }
    failures
        .iter()
        .map(|(codec, why)| format!("{}: {why}", codec_label(*codec)))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The first meaningful line of an ffmpeg probe failure, without the
/// `[h264_nvenc @ 000001f3c01124c0] ` prefix: the reason is shown to the user
/// in the encoder dropdown, and a raw pointer address is only noise there.
fn probe_reason(text: &str) -> String {
    let line = text
        .lines()
        .map(strip_ffmpeg_tag)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| text.trim());
    line.to_string()
}

fn strip_ffmpeg_tag(line: &str) -> &str {
    let mut rest = line.trim();
    // Nested tags happen: "[vost#0:0/h264_nvenc @ 0x..] [enc:h264_nvenc @ 0x..] ".
    while rest.starts_with('[') {
        let Some(end) = rest.find(']') else { break };
        let tag = &rest[1..end];
        if !tag.contains(" @ ") {
            break;
        }
        rest = rest[end + 1..].trim_start();
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(backend: VideoBackend, codecs: Vec<VideoCodec>) -> VideoBackendCapability {
        VideoBackendCapability {
            backend,
            available: true,
            codecs,
            reason: None,
        }
    }

    #[test]
    fn auto_uses_the_first_usable_backend_and_keeps_software_fallbacks() {
        let settings = VideoSettings {
            codec: VideoCodec::H265,
            audio: AudioMode::Copy,
            ..VideoSettings::default()
        };
        let caps = vec![
            capability(VideoBackend::Nvidia, vec![VideoCodec::H264]),
            capability(VideoBackend::Intel, vec![VideoCodec::H265]),
        ];

        let plan = resolve_video_encoding(&settings, &caps).unwrap();

        assert_eq!(plan.resolved_backend, VideoBackend::Intel);
        assert_eq!(plan.attempts.len(), 4);
        assert_eq!(plan.attempts[0].audio, AudioMode::Copy);
        assert_eq!(plan.attempts[1].audio, AudioMode::Aac);
        assert_eq!(plan.attempts[2].backend, VideoBackend::Software);
    }

    #[test]
    fn explicit_unavailable_backend_is_an_error() {
        let settings = VideoSettings {
            codec: VideoCodec::H265,
            hardware_acceleration: HardwareAcceleration::Nvidia,
            ..VideoSettings::default()
        };
        let error = resolve_video_encoding(&settings, &[]).unwrap_err();
        assert!(error.contains("NVIDIA NVENC"));
    }

    #[test]
    fn each_backend_gets_its_own_quality_and_filter_flags() {
        let settings = VideoSettings {
            codec: VideoCodec::H265,
            crf: 28,
            max_height: Some(1080),
            ..VideoSettings::default()
        };

        let nvenc = build_attempt(&settings, VideoBackend::Nvidia, AudioMode::Aac).unwrap();
        assert!(nvenc
            .output_flags
            .join(" ")
            .contains("hevc_nvenc -rc vbr -cq 28"));
        assert!(nvenc.output_flags.join(" ").contains("-preset p4"));

        let qsv = build_attempt(&settings, VideoBackend::Intel, AudioMode::Aac).unwrap();
        assert!(qsv.output_flags.join(" ").contains("-global_quality 28"));
        assert!(qsv.output_flags.join(" ").contains("format=nv12"));

        let amf = build_attempt(&settings, VideoBackend::Amd, AudioMode::Aac).unwrap();
        assert!(amf.output_flags.join(" ").contains("-rc qvbr"));

        let vaapi = build_attempt(&settings, VideoBackend::Vaapi, AudioMode::Aac).unwrap();
        assert!(vaapi.input_flags.join(" ").contains("-vaapi_device"));
        assert!(vaapi.output_flags.join(" ").contains("hwupload"));

        let videotoolbox =
            build_attempt(&settings, VideoBackend::VideoToolbox, AudioMode::Aac).unwrap();
        assert!(videotoolbox.output_flags.join(" ").contains("-q:v"));
    }

    #[test]
    fn detection_requires_a_successful_device_probe() {
        let encoders = " V....D h264_nvenc NVIDIA\n V....D hevc_nvenc NVIDIA\n";
        let caps = detect_video_backends_for(&[VideoBackend::Nvidia], encoders, &|attempt| {
            if attempt.output_flags.iter().any(|flag| flag == "h264_nvenc") {
                Ok(())
            } else {
                Err("No capable devices found".into())
            }
        });
        let nvidia = caps
            .iter()
            .find(|capability| capability.backend == VideoBackend::Nvidia)
            .unwrap();
        assert_eq!(nvidia.codecs, vec![VideoCodec::H264]);
        assert!(nvidia.available);
    }

    #[test]
    fn differing_codec_failures_keep_their_codec_prefix() {
        let encoders = " V....D h264_nvenc N\n V....D hevc_nvenc N\n";
        let caps = detect_video_backends_for(&[VideoBackend::Nvidia], encoders, &|attempt| {
            if attempt.output_flags.iter().any(|flag| flag == "h264_nvenc") {
                Err("[h264_nvenc @ 0x1] No capable devices found".into())
            } else {
                Err("[hevc_nvenc @ 0x1] Codec not supported".into())
            }
        });
        assert_eq!(
            caps[0].reason.as_deref(),
            Some("H.264: No capable devices found; H.265: Codec not supported")
        );
    }

    #[test]
    fn an_unusable_backend_reports_why_without_ffmpeg_noise() {
        let encoders = " V....D h264_nvenc NVIDIA\n";
        let caps = detect_video_backends_for(&[VideoBackend::Nvidia], encoders, &|_| {
            Err(concat!(
                "[h264_nvenc @ 000001f3c01124c0] Driver does not support the required ",
                "nvenc API version. Required: 13.1 Found: 12.2\n",
                "[h264_nvenc @ 000001f3c01124c0] The minimum required Nvidia driver is 610.00"
            )
            .into())
        });
        let nvidia = &caps[0];
        assert!(!nvidia.available);
        assert_eq!(
            nvidia.reason.as_deref(),
            Some(concat!(
                "Driver does not support the required nvenc API version. ",
                "Required: 13.1 Found: 12.2"
            ))
        );
    }

    #[test]
    fn a_backend_missing_from_the_build_says_so() {
        let caps = detect_video_backends_for(&[VideoBackend::Nvidia], "", &|_| Ok(()));
        assert!(!caps[0].available);
        assert_eq!(
            caps[0].reason.as_deref(),
            Some("Encoder not included in this FFmpeg build.")
        );
    }

    #[test]
    fn nested_ffmpeg_tags_are_stripped() {
        assert_eq!(
            strip_ffmpeg_tag("[vost#0:0/h264_amf @ 0x1] [enc:h264_amf @ 0x2] Failed to create"),
            "Failed to create"
        );
        // A bracketed word without an address is real message text, not a tag.
        assert_eq!(
            strip_ffmpeg_tag("[warning] disk full"),
            "[warning] disk full"
        );
    }

    #[test]
    fn fallback_respects_the_failure_kind() {
        let settings = VideoSettings {
            codec: VideoCodec::H264,
            audio: AudioMode::Copy,
            ..VideoSettings::default()
        };
        let plan = resolve_video_encoding(
            &settings,
            &[capability(VideoBackend::Nvidia, vec![VideoCodec::H264])],
        )
        .unwrap();

        let mut attempted = HashSet::from([0]);
        assert_eq!(
            next_attempt_index(&plan, 0, "No capable devices found", &attempted),
            Some(2)
        );
        assert_eq!(
            next_attempt_index(&plan, 0, "Could not write header", &attempted),
            Some(1)
        );
        attempted.insert(2);
        assert_eq!(
            next_attempt_index(&plan, 2, "Invalid data found", &attempted),
            None
        );
    }
}
