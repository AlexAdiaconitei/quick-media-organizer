# Changelog

This file records user-visible changes. Before publishing a version, move its
entries out of `Unreleased` and into a `## [x.y.z]` section.

## [Unreleased]

## [0.2.0] - 2026-08-29

### Added

- Added batch image and video optimization with format, quality, resolution,
  frame-rate and metadata controls.
- Added a Standard installer with FFmpeg and FFprobe included, plus a smaller
  Lite installer that uses an FFmpeg installation from `PATH`.
- Added per-fork in-app updates, signed update artifacts and links back to the
  repository that built the application.
- Added keyboard-first batch settings, progress reporting and cancellation.

### Changed

- Improved scanning, thumbnail loading and media caches for large folders.
- Preserved EXIF metadata when converting images.
- Moved the frontend toolchain to pnpm and made `pnpm dev` start the desktop
  application.
- Batch optimization now shows the folder it is working on and opens on the one
  the editor has open, with a "Change folder" button to point it elsewhere. The
  panel also follows the editor when the open folder changes, which it did not
  do when the folder was opened after the application started.
- Added Windows and Linux checks, installer startup smoke tests and guarded
  release publication.

### Fixed

- Prevented source-file loss when conversion, cleanup or cancellation fails.
- Fixed stalled batch jobs, unbounded caches and temporary-file cleanup.
- Fixed trimmed-file navigation and opening results in the default application.
- Fixed FFmpeg discovery outside the process `PATH` and suppressed console
  windows on Windows.
- Fixed application startup when updater signing keys are not configured.
- The first-run welcome screen no longer comes back after it was dismissed for
  good. Settings are written atomically and read field by field, so an
  interrupted write or one unreadable value no longer resets every preference,
  which also cost favourite folders and the saved batch settings. A settings
  file that cannot be parsed at all is kept as `settings.json.corrupt` instead
  of being overwritten.
- Fixed the batch folder picker pulling in subfolders by default.
- "Include subfolders" in batch optimization now re-scans the folders already
  loaded, so the item count changes when the switch is flipped. It previously
  only affected the next folder added, which made it look inert.
- Recursive batch scans no longer queue the output subfolder, so files produced
  by an earlier run are not optimized again.
- The video encoder dropdown now lists hardware encoders that were detected but
  cannot be used, together with the reason, instead of hiding them. A machine
  with a GPU no longer looks like it only has a CPU encoder.
- Fixed arrow-key navigation being swallowed by the focused rename field.
- Fixed the progress counter not decreasing when navigating backwards.
- Moved toasts clear of the toolbar buttons and made every toast dismissible.
