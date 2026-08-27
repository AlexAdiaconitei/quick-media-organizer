//! Carries EXIF across an image conversion.
//!
//! ffmpeg does not write EXIF when it encodes a still: a `.heic` converted to
//! `.jpg` comes out with no capture date and no GPS however `keep_metadata` is
//! set, because `-map_metadata` only moves container-level tags and the mjpeg
//! and libwebp encoders write none. So the block is read from the source
//! (`kamadak-exif` reads HEIF/JPEG/PNG/WebP/TIFF containers) and spliced into
//! the converted file (`img-parts` rewrites JPEG/PNG/WebP containers without
//! touching the pixels).
//!
//! A photo without readable EXIF, or whose target container cannot hold it, is
//! still a valid conversion. Once an EXIF block has been found, however, a
//! rewrite failure is reported so `keep_metadata` never succeeds deceptively.

use std::fs;
use std::io::BufReader;
use std::path::Path;

use img_parts::{Bytes, DynImage, ImageEXIF};

/// Output formats whose container has somewhere to put an EXIF block.
///
/// AVIF is deliberately absent: it is ISOBMFF, which `img-parts` does not
/// rewrite. `is_metadata_preserved` reports that to the UI so the checkbox
/// cannot promise something this build does not do.
pub fn can_carry_exif(extension: &str) -> bool {
    matches!(extension, "jpg" | "jpeg" | "png" | "webp")
}

/// `img-parts` splices a JPEG's EXIF segment in at a fixed index and panics on
/// a file with fewer segments than that. Anything ffmpeg writes has far more
/// (quantisation tables, frame header, scan), but the input here is a file on
/// disk, so it is checked rather than assumed: a panic in a worker would take
/// the whole batch job down with it.
fn can_hold_a_segment(image: &DynImage) -> bool {
    match image {
        DynImage::Jpeg(jpeg) => jpeg.segments().len() >= 3,
        _ => true,
    }
}

/// The TIFF block holding the source's EXIF, whatever container it came in.
fn read_exif_blob(source: &Path) -> Option<Vec<u8>> {
    let file = fs::File::open(source).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let blob = exif.buf();
    (!blob.is_empty()).then(|| blob.to_vec())
}

/// Copies the source's EXIF onto `target`, in place.
///
/// Returns whether anything was written. Errors are only returned for a target
/// that could not be rewritten after its metadata was resolved; a missing or
/// unreadable source block is simply "nothing to copy".
pub fn copy_exif(source: &Path, target: &Path) -> Result<bool, String> {
    let extension = target
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !can_carry_exif(&extension) {
        return Ok(false);
    }

    let Some(blob) = read_exif_blob(source) else {
        return Ok(false);
    };

    let encoded = fs::read(target).map_err(|e| format!("Cannot read the converted file: {e}"))?;
    let Some(mut image) = DynImage::from_bytes(Bytes::from(encoded))
        .map_err(|e| format!("Cannot parse the converted file: {e}"))?
    else {
        return Ok(false);
    };

    // Never overwrite what the encoder itself produced.
    if image.exif().is_some() {
        return Ok(false);
    }
    if !can_hold_a_segment(&image) {
        return Ok(false);
    }
    image.set_exif(Some(Bytes::from(blob)));

    // Written beside the target and renamed over it, so a failure half way
    // through cannot leave a truncated image where a valid one was.
    let staged = target.with_extension(format!("{extension}.qmo-exif"));
    let file =
        fs::File::create(&staged).map_err(|e| format!("Cannot write the converted file: {e}"))?;
    if let Err(error) = image.encoder().write_to(file) {
        let _ = fs::remove_file(&staged);
        return Err(format!("Cannot write the converted file: {error}"));
    }
    fs::rename(&staged, target).map_err(|e| {
        let _ = fs::remove_file(&staged);
        format!("Cannot replace the converted file: {e}")
    })?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_containers_that_can_hold_exif_are_offered() {
        assert!(can_carry_exif("jpg"));
        assert!(can_carry_exif("webp"));
        assert!(can_carry_exif("png"));
        // ISOBMFF: img-parts cannot rewrite these, so the UI must not claim it.
        assert!(!can_carry_exif("avif"));
        assert!(!can_carry_exif("gif"));
    }

    /// The whole point of the module: a JPEG that ffmpeg wrote without EXIF
    /// comes back out with the source's block attached.
    #[test]
    fn exif_is_spliced_into_a_jpeg_that_had_none() {
        let dir = tempfile::tempdir().expect("temp dir");

        let source = dir.path().join("source.jpg");
        fs::write(&source, jpeg_with_exif()).expect("write source");
        let target = dir.path().join("converted.jpg");
        fs::write(&target, jpeg_without_exif()).expect("write target");

        assert!(copy_exif(&source, &target).expect("copy"));

        let carried = read_exif_blob(&target).expect("the converted file now has EXIF");
        assert_eq!(carried, read_exif_blob(&source).expect("source EXIF"));
    }

    #[test]
    fn a_source_without_exif_leaves_the_target_alone() {
        let dir = tempfile::tempdir().expect("temp dir");

        let source = dir.path().join("source.jpg");
        fs::write(&source, jpeg_without_exif()).expect("write source");
        let target = dir.path().join("converted.jpg");
        fs::write(&target, jpeg_without_exif()).expect("write target");

        assert!(!copy_exif(&source, &target).expect("copy"));
        assert_eq!(fs::read(&target).unwrap(), jpeg_without_exif());
    }

    /// A JPEG shaped like something an encoder emits: SOI, JFIF, then the
    /// quantisation/frame/scan segments. The pixels are irrelevant, but the
    /// segment *count* is not — `img-parts` inserts EXIF at a fixed index.
    fn jpeg_without_exif() -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];

        // APP0/JFIF, so the file is recognisably a JPEG to every parser.
        out.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        out.extend_from_slice(b"JFIF\0");
        out.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);

        // COM x3: stand-ins for DQT/SOF/DHT. Same effect on the segment list,
        // without hand-writing Huffman tables.
        for note in [b"one", b"two", b"tre"] {
            out.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x05]);
            out.extend_from_slice(note);
        }

        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    /// The same file with an APP1 segment holding a little-endian TIFF header
    /// and one IFD entry (Orientation = 1).
    fn jpeg_with_exif() -> Vec<u8> {
        let tiff = tiff_block();
        let payload_len = (b"Exif\0\0".len() + tiff.len() + 2) as u16;

        let mut out = vec![0xFF, 0xD8];
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&payload_len.to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(&tiff);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    fn tiff_block() -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0"); // little endian, magic 42
        tiff.extend_from_slice(&8u32.to_le_bytes()); // offset of IFD0
        tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // count
        tiff.extend_from_slice(&1u16.to_le_bytes()); // value: normal
        tiff.extend_from_slice(&0u16.to_le_bytes()); // padding to 4 bytes
        tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        tiff
    }
}
