//! Disk layout for binary clipboard payloads (images, files-as-blobs).
//!
//! Layout (Windows):
//!   %APPDATA%\Klipo\blobs\<sha[:2]>\<sha>.<ext>      ← original payload
//!   %APPDATA%\Klipo\thumbs\<sha>-192.webp            ← lazy thumbnail
//!
//! `blob_path` stored in the DB is **relative** to the blobs root
//! (e.g. `0a/0a3f...c2.png`); the absolute path is resolved at runtime via
//! [`Storage::blob_root`]. This keeps the DB portable across users / paths.
//!
//! Hard caps:
//!   - 50 MB per blob (`docs/storage.md` §6). Larger payloads dropped at
//!     watcher time with a toast — never reach this module.
//!   - Two-level sharding by first 2 hex chars keeps any one folder under
//!     ~16k files even at 4M total.
//!
//! Concurrency: writes go through `tokio::fs`; we don't lock — the partial
//! unique index on `clips.content_hash` ensures we never *insert two rows*
//! for the same hash, but two different rows could theoretically race the
//! same path. They produce identical bytes (same hash), so last-write-wins
//! is harmless. We still tolerate "file already exists" on rename.

use std::path::{Path, PathBuf};

use image::ImageFormat;

use super::error::{StorageError, StorageResult};
use super::Storage;

/// Hard cap, mirrors `docs/storage.md`. The watcher should drop earlier; this
/// is the safety net.
pub const MAX_BLOB_BYTES: usize = 50 * 1024 * 1024;

const THUMB_LONG_EDGE: u32 = 192;

impl Storage {
    /// Absolute path to the blobs root. Constructed lazily from the DB path;
    /// the DB lives at `<root>/klipo.db`, blobs at `<root>/blobs`.
    pub fn blob_root(&self) -> Option<PathBuf> {
        self.db_dir().map(|d| d.join("blobs"))
    }

    pub fn thumb_root(&self) -> Option<PathBuf> {
        self.db_dir().map(|d| d.join("thumbs"))
    }

    /// Resolve a stored relative `blob_path` to an absolute filesystem path.
    pub fn resolve_blob(&self, relative: &str) -> Option<PathBuf> {
        self.blob_root().map(|r| r.join(relative))
    }

    pub fn resolve_thumb(&self, hash: &str) -> Option<PathBuf> {
        self.thumb_root()
            .map(|r| r.join(format!("{hash}-{THUMB_LONG_EDGE}.webp")))
    }
}

/// Build the relative blob path used inside the DB and on disk.
pub fn relative_blob_path(hash: &str, ext: &str) -> String {
    let prefix = &hash[..hash.len().min(2)];
    format!("{prefix}/{hash}.{ext}")
}

/// Write `bytes` to the blob layout under `blob_root` and return the
/// relative path (e.g. `0a/0a3f...c2.png`).
///
/// `ext` is the file extension WITHOUT the leading dot (`"png"`, `"jpg"`).
/// Hash is the SHA-256 hex of the original payload.
pub async fn write_blob(
    blob_root: &Path,
    hash: &str,
    ext: &str,
    bytes: &[u8],
) -> StorageResult<String> {
    if bytes.len() > MAX_BLOB_BYTES {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            format!("payload {} bytes exceeds 50MB cap", bytes.len()),
        )));
    }

    let rel = relative_blob_path(hash, ext);
    let abs = blob_root.join(&rel);
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // Idempotent: if the blob already exists, leave it alone — content is
    // hash-addressed so any existing file is byte-identical.
    if !tokio::fs::try_exists(&abs).await.unwrap_or(false) {
        tokio::fs::write(&abs, bytes).await?;
    }
    Ok(rel)
}

/// Generate a 192-px-long-edge WebP thumbnail and write it to the thumbs
/// directory. Returns the absolute path of the thumbnail on success.
///
/// `image_bytes` must be a decodable image (PNG/JPEG/BMP). On decode failure
/// we log and return Ok(None) — thumbnail is best-effort, never blocks
/// the capture pipeline.
pub async fn write_thumbnail(
    thumb_root: &Path,
    hash: &str,
    image_bytes: Vec<u8>,
) -> StorageResult<Option<PathBuf>> {
    let path = thumb_root.join(format!("{hash}-{THUMB_LONG_EDGE}.webp"));
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(Some(path));
    }

    let path_for_task = path.clone();
    let result =
        tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>, image::ImageError> {
            let img = image::load_from_memory(&image_bytes)?;
            let thumb = img.thumbnail(THUMB_LONG_EDGE, THUMB_LONG_EDGE);
            thumb.save_with_format(&path_for_task, ImageFormat::WebP)?;
            Ok(Some(path_for_task))
        })
        .await;

    match result {
        Ok(Ok(p)) => Ok(p),
        Ok(Err(e)) => {
            tracing::warn!(target: "klipo::blob", error = %e, hash, "thumbnail decode/encode failed");
            Ok(None)
        }
        Err(join_err) => {
            tracing::warn!(target: "klipo::blob", error = %join_err, "thumbnail task join error");
            Ok(None)
        }
    }
}

/// Re-encode arbitrary image bytes (BMP from clipboard, JPEG from browser,
/// etc.) into PNG with stripped metadata. Returns `(png_bytes, sha256_hex)`.
///
/// Why re-encode rather than store as-is:
///   - Strip EXIF / source-app metadata for privacy.
///   - Normalize so dedup hash matches across same-content-different-encoding
///     screenshots (Win+Shift+S vs Snip & Sketch produce slightly different
///     bytes for the same pixel grid).
pub fn reencode_to_png(input: &[u8]) -> StorageResult<(Vec<u8>, String)> {
    use sha2::{Digest, Sha256};
    use std::io::Cursor;

    let img = image::load_from_memory(input)
        .map_err(|e| StorageError::Io(std::io::Error::other(e.to_string())))?;

    let mut out = Vec::with_capacity(input.len());
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| StorageError::Io(std::io::Error::other(e.to_string())))?;

    let mut hasher = Sha256::new();
    hasher.update(&out);
    let hash = format!("{:x}", hasher.finalize());

    Ok((out, hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn relative_blob_path_shards_by_first_two_chars() {
        assert_eq!(relative_blob_path("0a3fdef", "png"), "0a/0a3fdef.png");
        assert_eq!(relative_blob_path("ff", "bin"), "ff/ff.bin");
    }

    #[tokio::test]
    async fn write_blob_idempotent() {
        let dir = TempDir::new().unwrap();
        let p1 = write_blob(dir.path(), "deadbeef", "png", b"hello world")
            .await
            .unwrap();
        let p2 = write_blob(dir.path(), "deadbeef", "png", b"hello world")
            .await
            .unwrap();
        assert_eq!(p1, p2);
        assert_eq!(p1, "de/deadbeef.png");
        assert!(dir.path().join(&p1).exists());
    }

    #[tokio::test]
    async fn write_blob_rejects_oversize() {
        let dir = TempDir::new().unwrap();
        let big = vec![0u8; MAX_BLOB_BYTES + 1];
        let result = write_blob(dir.path(), "ff", "bin", &big).await;
        assert!(result.is_err());
    }

    #[test]
    fn reencode_png_produces_valid_png() {
        // Build a 2×2 RGBA image as raw PNG using image crate, then run it
        // through reencode_to_png to validate roundtrip.
        use image::{ImageBuffer, Rgba};
        let img = ImageBuffer::from_fn(2, 2, |x, y| {
            Rgba([(x * 100) as u8, (y * 100) as u8, 200, 255])
        });
        let mut original = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut original), ImageFormat::Png)
            .unwrap();

        let (re_png, hash) = reencode_to_png(&original).unwrap();
        assert!(!re_png.is_empty());
        assert_eq!(hash.len(), 64);
        // Reencoding twice produces identical hash.
        let (_, hash2) = reencode_to_png(&re_png).unwrap();
        assert_eq!(hash, hash2);
    }
}
