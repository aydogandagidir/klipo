//! Clipboard subsystem.
//!
//! Phase B M3 splits into three sub-milestones:
//!   - **M3.0:** pure-logic primitives — sensitive detection, foreground app.
//!   - **M3.1 (current):** native Windows watcher (`AddClipboardFormatListener`
//!     + message pump) wired to a Tokio pipeline that persists captures.
//!   - **M3.2:** image / file / rtf / html capture (currently text-only).
//!
//! macOS watcher (`watcher_macos`) lands in v0.2 (Phase C).

pub mod pipeline;
pub mod sensitive;
pub mod source_app;

#[cfg(windows)]
pub mod normalize;

#[cfg(windows)]
pub mod paste;

#[cfg(windows)]
pub mod watcher_windows;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::storage::Storage;

/// A captured clipboard event, before storage normalization.
///
/// Producers (`watcher_*`) build these and forward them through an mpsc
/// channel to `pipeline::run`, which runs:
///   excluded-apps filter → sensitive scan → hash → `Storage::insert_clip`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEvent {
    pub kind: CapturedKind,
    /// Raw text content if `kind == Text|Rtf|Html`. `None` otherwise.
    pub text: Option<String>,
    /// Raw bytes for image/file payloads. `None` for text-only.
    pub bytes: Option<Vec<u8>>,
    pub size_bytes: u64,
    pub source_app: Option<String>,
    pub source_window_title: Option<String>,
    /// Captured at unix-ms; producer-side timestamp.
    pub captured_at_ms: i64,
}

/// What the OS told us this clipboard payload was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapturedKind {
    Text,
    Image,
    File,
    Rtf,
    Html,
}

impl CapturedKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CapturedKind::Text => "text",
            CapturedKind::Image => "image",
            CapturedKind::File => "file",
            CapturedKind::Rtf => "rtf",
            CapturedKind::Html => "html",
        }
    }
}

/// Cross-platform entry point. Spawns the OS watcher + the persistence pipeline.
///
/// On Windows: spawns a dedicated thread for the message pump and a tokio
/// task for the persistence pipeline.
///
/// On other platforms (M3.1): logs a warning and returns Ok(()) — Klipo
/// still runs, just without capturing. macOS support arrives with v0.2.
pub fn start(storage: Storage, app: AppHandle) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        watcher_windows::spawn(tx)?;
        tauri::async_runtime::spawn(pipeline::run(rx, storage, app));
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (storage, app);
        tracing::warn!(
            target: "klipo::watcher",
            "clipboard watcher not implemented on this platform yet (macOS arrives in v0.2)"
        );
        Ok(())
    }
}
