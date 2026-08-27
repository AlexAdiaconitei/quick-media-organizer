use filetime::FileTime;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct TimestampSnapshot {
    pub accessed: FileTime,
    pub modified: FileTime,
}

pub fn read_timestamps(path: &Path) -> io::Result<TimestampSnapshot> {
    let meta = fs::metadata(path)?;
    Ok(TimestampSnapshot {
        accessed: FileTime::from_last_access_time(&meta),
        modified: FileTime::from_last_modification_time(&meta),
    })
}

pub fn apply_timestamps(path: &Path, snap: &TimestampSnapshot) -> io::Result<()> {
    filetime::set_file_times(path, snap.accessed, snap.modified)
}

pub fn move_file_preserve(from: &Path, to: &Path) -> io::Result<()> {
    let snap = read_timestamps(from)?;

    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }

    match fs::rename(from, to) {
        Ok(()) => {
            apply_timestamps(to, &snap)?;
            Ok(())
        }
        Err(err) if is_cross_device(&err) => {
            fs::copy(from, to)?;
            apply_timestamps(to, &snap)?;
            if let Err(remove_err) = fs::remove_file(from) {
                let _ = fs::remove_file(to);
                return Err(remove_err);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub fn rollback_moves(moves: &[(PathBuf, PathBuf)]) {
    for (from, to) in moves.iter().rev() {
        if from.exists() && !to.exists() {
            let _ = move_file_preserve(from, to);
        }
    }
}

pub fn execute_moves(sources: &[PathBuf], dest_names: &[String], dest_dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    if sources.len() != dest_names.len() {
        return Err("Internal error: source/destination count mismatch.".into());
    }

    let mut completed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (source, name) in sources.iter().zip(dest_names.iter()) {
        let dest = dest_dir.join(name);
        if let Err(err) = move_file_preserve(source, &dest) {
            rollback_moves(
                &completed
                    .iter()
                    .map(|(from, to)| (from.clone(), to.clone()))
                    .collect::<Vec<_>>(),
            );
            return Err(format!("Failed to move {}: {err}", source.display()));
        }
        completed.push((dest, source.clone()));
    }
    Ok(completed)
}

/// A rename that crossed a volume boundary, so the caller can fall back to
/// copy + delete.
///
/// The codes are per-platform and do not overlap: Windows reports
/// `ERROR_NOT_SAME_DEVICE` (17) from `MoveFileExW`, while Unix reports `EXDEV`
/// (18 on Linux/macOS, 152 on Solaris-likes). Matching them all everywhere
/// would be wrong -- 18 on Windows is `ERROR_NO_MORE_FILES`.
fn is_cross_device(err: &io::Error) -> bool {
    #[cfg(windows)]
    {
        matches!(err.raw_os_error(), Some(17))
    }
    #[cfg(not(windows))]
    {
        matches!(err.raw_os_error(), Some(18) | Some(152))
    }
}
