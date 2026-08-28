use std::fs;
use std::path::{Path, PathBuf};

use super::{BatchItemState, BatchJobStatus, VideoBackend};

/// Writes both a readable report and a spreadsheet-friendly report. Reports
/// are append-only artifacts: rerunning a job gets a new job id and never
/// overwrites a previous result.
pub fn write_batch_report(dir: &Path, job: &BatchJobStatus) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(dir).map_err(|error| format!("Could not create report folder: {error}"))?;

    let stem = format!("batch-{}", job.job_id);
    let markdown_path = dir.join(format!("{stem}.md"));
    let csv_path = dir.join(format!("{stem}.csv"));

    fs::write(&markdown_path, markdown(job))
        .map_err(|error| format!("Could not write batch report: {error}"))?;
    fs::write(&csv_path, csv(job))
        .map_err(|error| format!("Could not write batch CSV report: {error}"))?;

    Ok((markdown_path, csv_path))
}

fn markdown(job: &BatchJobStatus) -> String {
    let mut output = format!(
        "# Batch report {}\n\n- Started: {}\n- Finished: {}\n- Converted: {}\n- Skipped: {}\n- Failed: {}\n- Bytes before: {}\n- Bytes after: {}\n\n## Files\n\n",
        job.job_id,
        job.started_at,
        job.finished_at.as_deref().unwrap_or("unknown"),
        job.done,
        job.skipped,
        job.failed,
        job.bytes_before,
        job.bytes_after,
    );

    for item in &job.items {
        output.push_str(&format!(
            "- **{}** — `{}` — {}",
            escape_markdown(&item.file_name),
            state_name(item.state),
            escape_markdown(&item.source_path),
        ));
        if let Some(error) = &item.error {
            output.push_str(&format!(" — {}", escape_markdown(error)));
        }
        if let Some(backend) = item.encoder_backend {
            output.push_str(&format!(" — encoder: {}", backend_name(backend)));
        }
        if let Some(reason) = &item.fallback_reason {
            output.push_str(&format!(" — fallback: {}", escape_markdown(reason)));
        }
        output.push('\n');
    }
    output
}

fn csv(job: &BatchJobStatus) -> String {
    let mut output =
        "file_name,source_path,state,size_before,size_after,output_path,encoder_backend,fallback_reason,error\n".to_string();
    for item in &job.items {
        let fields = [
            item.file_name.clone(),
            item.source_path.clone(),
            state_name(item.state).to_string(),
            item.size_before.to_string(),
            item.size_after
                .map(|value| value.to_string())
                .unwrap_or_default(),
            item.output_path.clone().unwrap_or_default(),
            item.encoder_backend
                .map(backend_name)
                .unwrap_or_default()
                .to_string(),
            item.fallback_reason.clone().unwrap_or_default(),
            item.error.clone().unwrap_or_default(),
        ];
        output.push_str(&fields.map(|value| csv_field(&value)).join(","));
        output.push('\n');
    }
    output
}

fn backend_name(backend: VideoBackend) -> &'static str {
    match backend {
        VideoBackend::Software => "software",
        VideoBackend::Nvidia => "nvidia_nvenc",
        VideoBackend::Intel => "intel_qsv",
        VideoBackend::Amd => "amd_amf",
        VideoBackend::VideoToolbox => "apple_videotoolbox",
        VideoBackend::Vaapi => "vaapi",
    }
}

fn state_name(state: BatchItemState) -> &'static str {
    match state {
        BatchItemState::Pending => "pending",
        BatchItemState::Running => "running",
        BatchItemState::Done => "done",
        BatchItemState::Skipped => "skipped",
        BatchItemState::Failed => "failed",
        BatchItemState::Cancelled => "cancelled",
    }
}

fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn escape_markdown(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::{BatchItemStatus, BatchMediaType, VideoBackend};

    #[test]
    fn writes_markdown_and_csv_with_failure_details() {
        let dir = tempfile::tempdir().unwrap();
        let job = BatchJobStatus {
            job_id: "job-test-1".into(),
            running: false,
            cancelled: false,
            total: 1,
            done: 0,
            failed: 1,
            skipped: 0,
            bytes_before: 42,
            bytes_after: 0,
            started_at: "2026-08-27T10:00:00Z".into(),
            finished_at: Some("2026-08-27T10:01:00Z".into()),
            output_dir: None,
            replaces_originals: false,
            items: vec![BatchItemStatus {
                id: "broken.mov".into(),
                source_path: "C:\\camera\\broken.mov".into(),
                file_name: "broken.mov".into(),
                media_type: BatchMediaType::Video,
                state: BatchItemState::Failed,
                progress: 0.0,
                size_before: 42,
                size_after: None,
                output_path: None,
                error: Some("bad \"codec\"".into()),
                encoder_backend: Some(VideoBackend::Software),
                fallback_reason: Some("GPU unavailable".into()),
            }],
            replacements: vec![],
            finalized: false,
        };

        let (markdown, csv) = write_batch_report(dir.path(), &job).unwrap();
        let markdown = fs::read_to_string(markdown).unwrap();
        let csv = fs::read_to_string(csv).unwrap();

        assert!(markdown.contains("bad \"codec\""));
        assert!(csv.contains("\"bad \"\"codec\"\"\""));
        assert!(csv.contains("\"C:\\camera\\broken.mov\""));
        assert!(markdown.contains("encoder: software"));
        assert!(csv.contains("\"GPU unavailable\""));
    }
}
