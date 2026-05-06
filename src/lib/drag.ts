import { startDrag } from "@crabnebula/tauri-plugin-drag";

import type { Clip } from "@/lib/ipc";
import { resolveBlobAbsolute } from "@/lib/ipc";

/**
 * Start a native OS drag-and-drop operation for a clip.
 *
 * Why this exists: Chromium-based apps (any browser shell, Discord, Slack,
 * Notion, Obsidian, etc.) silently reject `Ctrl+V` of file payloads.
 * Native drag-and-drop is the Chromium-blessed path for moving a file from
 * outside the browser to a page's drop zone. The plugin opens an OS-level
 * drag with real file paths; once the user releases over a target window,
 * that window receives a normal drop event.
 *
 * Supported clip kinds:
 *   - `file`  → drag the path list straight from `text_content` (JSON).
 *   - `image` → resolve the on-disk PNG blob path and drag that.
 *   - everything else → null/no-op (text drag goes through normal copy).
 *
 * Returns whether the drag was actually started so the caller can show a
 * hint when it skipped.
 */
export async function startNativeDrag(clip: Clip): Promise<boolean> {
  if (clip.kind === "file" && clip.text_content) {
    let paths: string[] = [];
    try {
      const parsed: unknown = JSON.parse(clip.text_content);
      if (Array.isArray(parsed)) {
        paths = parsed.map(String).filter((p) => p.length > 0);
      }
    } catch {
      return false;
    }
    if (paths.length === 0) return false;
    await startDrag({ item: paths, icon: "" });
    return true;
  }

  if (clip.kind === "image" && clip.blob_path) {
    const abs = await resolveBlobAbsolute(clip.blob_path);
    await startDrag({ item: [abs], icon: "" });
    return true;
  }

  return false;
}
