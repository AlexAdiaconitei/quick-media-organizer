//! Persistent checkpoint for the one active batch job.
//!
//! The runner owns execution. This module only records enough state to resume
//! pending files after the desktop process exits, without replanning output
//! names or losing replacement backups needed by undo.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::runner::{EncodePlan, PlannedItem};
use super::{BatchItemState, BatchJobStatus, BatchSettings};

const CHECKPOINT_DIR: &str = "batch-jobs";
const ACTIVE_FILE: &str = "active.json";
const PREVIOUS_FILE: &str = "active.previous.json";
const NEXT_FILE: &str = "active.next.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCheckpoint {
    pub job: BatchJobStatus,
    pub settings: BatchSettings,
    /// Kept index-aligned with `job.items`. Terminal items retain their plan
    /// so the checkpoint remains a complete audit record.
    pub plans: Vec<Option<EncodePlan>>,
}

impl BatchCheckpoint {
    pub fn new(job: BatchJobStatus, settings: BatchSettings, planned: &[PlannedItem]) -> Self {
        Self {
            job,
            settings,
            plans: planned.iter().map(|item| item.plan.clone()).collect(),
        }
    }

    /// Converts a process-interrupted checkpoint back into runner input.
    /// Completed, skipped and failed files stay terminal. Only pending or
    /// formerly-running files receive a plan.
    pub fn prepare_resume(&mut self) -> Vec<PlannedItem> {
        self.job.running = true;
        self.job.cancelled = false;
        self.job.finished_at = None;

        self.job
            .items
            .iter_mut()
            .enumerate()
            .map(|(index, status)| {
                let plan = self.plans.get(index).and_then(Option::as_ref);
                let resumable = matches!(
                    status.state,
                    BatchItemState::Pending | BatchItemState::Running
                );
                match (resumable, plan) {
                    (true, Some(plan)) => {
                        status.state = BatchItemState::Pending;
                        status.progress = 0.0;
                        status.error = None;
                        let _ = fs::remove_file(&plan.temp_output);
                        PlannedItem {
                            status: status.clone(),
                            plan: Some(plan.clone()),
                        }
                    }
                    (true, None) => {
                        status.state = BatchItemState::Failed;
                        status.progress = 0.0;
                        status.error = Some("The saved resume plan is missing.".into());
                        PlannedItem {
                            status: status.clone(),
                            plan: None,
                        }
                    }
                    (false, _) => PlannedItem {
                        status: status.clone(),
                        plan: None,
                    },
                }
            })
            .collect()
    }

    pub fn update_job(&mut self, job: BatchJobStatus) {
        let current_terminal = terminal_items(&self.job);
        let incoming_terminal = terminal_items(&job);
        if incoming_terminal < current_terminal
            || (!self.job.running && job.running)
            || (self.job.finalized && !job.finalized)
        {
            return;
        }
        self.job = job;
    }
}

fn terminal_items(job: &BatchJobStatus) -> usize {
    job.items
        .iter()
        .filter(|item| {
            matches!(
                item.state,
                BatchItemState::Done
                    | BatchItemState::Skipped
                    | BatchItemState::Failed
                    | BatchItemState::Cancelled
            )
        })
        .count()
}

#[derive(Debug, Clone)]
pub struct BatchCheckpointStore {
    directory: PathBuf,
}

impl BatchCheckpointStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            directory: app_data_dir.join(CHECKPOINT_DIR),
        }
    }

    pub fn save(&self, checkpoint: &BatchCheckpoint) -> Result<(), String> {
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("Could not create batch checkpoint folder: {error}"))?;
        let next = self.directory.join(NEXT_FILE);
        let active = self.directory.join(ACTIVE_FILE);
        let previous = self.directory.join(PREVIOUS_FILE);
        let content = serde_json::to_vec_pretty(checkpoint)
            .map_err(|error| format!("Could not serialize batch checkpoint: {error}"))?;
        fs::write(&next, content)
            .map_err(|error| format!("Could not write batch checkpoint: {error}"))?;

        if active.exists() {
            let _ = fs::copy(&active, &previous);
            fs::remove_file(&active)
                .map_err(|error| format!("Could not rotate batch checkpoint: {error}"))?;
        }
        fs::rename(&next, &active)
            .map_err(|error| format!("Could not activate batch checkpoint: {error}"))
    }

    pub fn load(&self) -> Result<Option<BatchCheckpoint>, String> {
        for path in [
            self.directory.join(ACTIVE_FILE),
            self.directory.join(PREVIOUS_FILE),
        ] {
            let Ok(content) = fs::read(&path) else {
                continue;
            };
            if let Ok(checkpoint) = serde_json::from_slice(&content) {
                return Ok(Some(checkpoint));
            }
        }
        Ok(None)
    }

    pub fn clear(&self) -> Result<(), String> {
        if !self.directory.exists() {
            return Ok(());
        }
        for name in [ACTIVE_FILE, PREVIOUS_FILE, NEXT_FILE] {
            let path = self.directory.join(name);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|error| format!("Could not remove batch checkpoint: {error}"))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::runner::new_job_status;
    use crate::batch::video_backend::VideoEncodeAttempt;
    use crate::batch::{AudioMode, BatchItemStatus, BatchMediaType, VideoBackend};

    fn status(id: &str, state: BatchItemState) -> BatchItemStatus {
        BatchItemStatus {
            id: id.into(),
            source_path: id.into(),
            file_name: id.into(),
            media_type: BatchMediaType::Video,
            state,
            progress: 0.5,
            size_before: 100,
            size_after: None,
            output_path: None,
            error: Some("old process ended".into()),
            encoder_backend: None,
            fallback_reason: None,
        }
    }

    fn plan(path: &str) -> EncodePlan {
        EncodePlan {
            input: PathBuf::from(path),
            temp_output: PathBuf::from(format!("{path}.tmp.mp4")),
            final_output: PathBuf::from(format!("{path}.out.mp4")),
            attempts: vec![VideoEncodeAttempt {
                backend: VideoBackend::Software,
                audio: AudioMode::Aac,
                input_flags: vec![],
                output_flags: vec![],
            }],
            resolved_backend: Some(VideoBackend::Software),
            media_type: BatchMediaType::Video,
            replace_original: false,
            backup: false,
            overwritten_target: None,
        }
    }

    #[test]
    fn store_recovers_from_the_previous_generation() {
        let dir = tempfile::tempdir().unwrap();
        let store = BatchCheckpointStore::new(dir.path());
        let checkpoint = BatchCheckpoint {
            job: new_job_status("resume-me".into(), vec![], None, false),
            settings: BatchSettings::default(),
            plans: vec![],
        };
        store.save(&checkpoint).unwrap();
        store.save(&checkpoint).unwrap();
        fs::write(store.directory.join(ACTIVE_FILE), b"broken").unwrap();

        assert_eq!(store.load().unwrap().unwrap().job.job_id, "resume-me");
    }

    #[test]
    fn resume_only_requeues_pending_and_running_items() {
        let mut job = new_job_status(
            "resume".into(),
            vec![
                status("done.mov", BatchItemState::Done),
                status("running.mov", BatchItemState::Running),
                status("pending.mov", BatchItemState::Pending),
            ],
            None,
            false,
        );
        job.done = 1;
        let mut checkpoint = BatchCheckpoint {
            job,
            settings: BatchSettings::default(),
            plans: vec![None, Some(plan("running.mov")), Some(plan("pending.mov"))],
        };

        let planned = checkpoint.prepare_resume();

        assert!(planned[0].plan.is_none());
        assert_eq!(planned[0].status.state, BatchItemState::Done);
        assert_eq!(planned[1].status.state, BatchItemState::Pending);
        assert_eq!(planned[2].status.state, BatchItemState::Pending);
        assert!(planned[1].status.error.is_none());
    }

    #[test]
    fn a_truncated_checkpoint_fails_the_unplanned_item_instead_of_losing_it() {
        let job = new_job_status(
            "truncated".into(),
            vec![status("pending.mov", BatchItemState::Pending)],
            None,
            false,
        );
        let mut checkpoint = BatchCheckpoint {
            job,
            settings: BatchSettings::default(),
            plans: vec![],
        };

        let planned = checkpoint.prepare_resume();

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].status.state, BatchItemState::Failed);
        assert!(planned[0].status.error.is_some());
    }

    #[test]
    fn a_late_worker_snapshot_cannot_move_the_checkpoint_backwards() {
        let mut current = new_job_status(
            "ordered".into(),
            vec![
                status("one.mov", BatchItemState::Done),
                status("two.mov", BatchItemState::Done),
            ],
            None,
            false,
        );
        current.done = 2;
        let mut checkpoint = BatchCheckpoint {
            job: current,
            settings: BatchSettings::default(),
            plans: vec![],
        };
        let mut stale = checkpoint.job.clone();
        stale.items[1].state = BatchItemState::Pending;
        stale.done = 1;

        checkpoint.update_job(stale);

        assert_eq!(checkpoint.job.done, 2);
        assert_eq!(checkpoint.job.items[1].state, BatchItemState::Done);
    }
}
