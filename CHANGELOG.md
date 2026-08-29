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
- Added Windows and Linux checks, installer startup smoke tests and guarded
  release publication.

### Fixed

- Prevented source-file loss when conversion, cleanup or cancellation fails.
- Fixed stalled batch jobs, unbounded caches and temporary-file cleanup.
- Fixed trimmed-file navigation and opening results in the default application.
- Fixed FFmpeg discovery outside the process `PATH` and suppressed console
  windows on Windows.
- Fixed application startup when updater signing keys are not configured.
- Fixed the batch folder picker pulling in subfolders by default.
- Fixed arrow-key navigation being swallowed by the focused rename field.
- Fixed the progress counter not decreasing when navigating backwards.
- Moved toasts clear of the toolbar buttons and made every toast dismissible.
