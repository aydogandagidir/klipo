# v0.1 Launch Drafts

Drafts for the post-v0.1 launch announcements. Edit / copy / discard as fits the platform.

> Maintainer notes:
>
> - These drafts intentionally don't compare Klipo to Maccy / Raycast / Ditto / 1Password by name — that's a deliberate constraint from the brand-voice rules. Talk about the gap, not the apps.
> - Don't post until you've run through the smoke-test checklist on a clean Windows VM. SmartScreen + first-install UX matters most for the first 100 visitors.
> - Replace `<RECORDED-DATE>` once you actually post.

---

## Show HN draft

**Title (≤80 chars):**

> Show HN: Klipo – cross-platform clipboard manager with auto-update + signed releases

**Body:**

```
Hey HN,

I built Klipo because every clipboard manager I tried was either platform-locked, paid, or frozen in 2014. Klipo is open-source (Apache-2.0), Windows-first today, macOS in the next minor — and the hard infra parts are already in place: signed Ed25519 update manifest, FTS5 search with Türkçe character folding, 13-pattern sensitive-content guard (API keys / credit cards / JWTs get a red border + paste confirm), per-app exclusion list seeded with common password managers.

What's actually shipped in v0.1:
- Capture text · image · file · RTF · HTML; SHA-256 dedup; native OS drag-and-drop out of the popup (so you can drag a file clip straight into a browser drop zone — Chromium silently rejects Ctrl+V of file payloads).
- Hotkey rebind, theme picker (light/dark/system, live), excluded-apps editor with "capture next foreground app", autostart toggle, wipe-all, full Settings UI.
- 4-step onboarding, tray icon, Ctrl+Q quit + relaunch instructions baked into onboarding/About/README.
- Auto-update via tauri-plugin-updater. Verified end-to-end live: v0.1.0 → v0.1.1 → v0.1.2 in production. Each tag push runs a GitHub Actions workflow that bundles MSI + NSIS, signs them with the project's Ed25519 key, generates `latest.json`, drafts a GitHub Release. After publish, the running binary fetches the manifest, verifies the signature, downloads the new installer, restarts. No EV cert yet (~$300/yr — planned).

Stack: Tauri 2 + React 18 + TypeScript 5 + Tailwind 3 + shadcn-style components; SQLite + sqlx 0.8 + FTS5; arboard for clipboard format conversion; tauri-plugin-drag for native OS drag.

Not in v0.1 yet: macOS port, sync (planned with libsodium + LWW-CRDT + HLC, opt-in E2E, both Cloudflare Workers cloud and self-host Docker — single protocol), AI transforms (BYO Anthropic/OpenAI/Ollama + hosted toggle), snippets engine, OCR (Tesseract).

Repo: https://github.com/aydogandagidir/klipo
Download: https://github.com/aydogandagidir/klipo/releases/latest

Happy to take feedback on threat model, perf budget, sync protocol design, or just rough edges in the popup.
```

---

## Product Hunt teaser

**Tagline (60 chars):**

> Cross-platform clipboard manager with signed auto-update.

**Description (260 chars):**

> Klipo is an open-source clipboard manager for Windows (macOS soon). Captures text, image, file, RTF, HTML — searches in milliseconds with Türkçe-aware FTS5 — flags secrets before paste — drags files into browser drop zones natively. Auto-update is signed and verified.

**First comment (the maker):**

> Hey PH 👋
>
> Klipo is the clipboard manager I wanted but couldn't find: cross-platform from day one, signed auto-updates so v0.1 → v0.1.x ships with zero manual installer downloads, and a privacy posture that takes secrets seriously (API keys / credit cards / JWTs get a red border and a paste-confirm dialog by default).
>
> What's there today (Windows, signed): capture text/image/file/RTF/HTML, FTS5 search with Türkçe character folding, native OS drag-and-drop out of the popup, hotkey rebind, theme picker, excluded-apps editor, autostart, wipe-all, 4-step onboarding tour, tray icon.
>
> What's coming next: macOS port (Faz C — `NSPasteboard` watcher, `Cmd+Option+V` hotkey, vibrancy-styled popup), opt-in end-to-end encrypted sync (both cloud and self-host, single protocol), AI transforms (BYO key + hosted toggle), snippets with variables, OCR.
>
> Apache-2.0, repo's at github.com/aydogandagidir/klipo. Bug reports, feature ideas, perf complaints all welcome — there's a SECURITY.md for vulnerability disclosure too.

---

## Reddit (r/Windows10, r/opensource, r/rust) — short form

> **Klipo v0.1 — open-source clipboard manager for Windows with signed auto-update**
>
> Just shipped the first release of Klipo, an Apache-2.0 clipboard manager written in Rust + Tauri 2 + React. Windows today, macOS in v0.2. Highlights:
>
> - **Captures everything**: text · image · file · RTF · HTML, with hash dedup
> - **Searches fast**: SQLite FTS5, BM25-ranked, Türkçe character folding works out of the box
> - **Privacy-aware**: 13-pattern auto-detect for API keys / credit cards / JWTs — red border + confirm before pasting; per-app exclusion seeded with common password managers
> - **Drags files out**: NSIS-side native OS drag, so you can drop a file clip into a Chromium-based app's drop zone (Ctrl+V of files silently rejected by Chromium for security; drag works)
> - **Auto-updates**: Ed25519-signed manifest, verified live in production over three releases now
>
> No telemetry, no account, no sync server — sync arrives in v0.3 as opt-in E2E (cloud + self-host options).
>
> Download: https://github.com/aydogandagidir/klipo/releases/latest
> Source: https://github.com/aydogandagidir/klipo

---

## Twitter / X thread

**Tweet 1:**

> Just shipped v0.1 of Klipo, an open-source clipboard manager for Windows.
>
> What makes it different:
> 🔒 Detects API keys / credit cards / JWTs, asks before pasting
> 🧠 FTS5 search works in Türkçe out of the box
> 🚀 Native OS drag-and-drop into browser drop zones
> 🔄 Signed auto-updates
>
> github.com/aydogandagidir/klipo

**Tweet 2:**

> Built on Tauri 2 + React 18 + Rust + SQLite. Ships at 3.8 MB (NSIS installer).
>
> macOS port lands in v0.2. Sync (libsodium + LWW-CRDT, opt-in, both cloud and self-host) lands in v0.3.
>
> Apache-2.0. No telemetry, no account.

**Tweet 3 (technical):**

> The hard parts that already work:
>
> • Tauri-signed update manifest, end-to-end verified across 3 releases
> • Sensitive-content guard with 13 regex patterns, runs per-clip
> • Native drag-and-drop because Chromium silently rejects Ctrl+V of files
> • Türkçe character folding in FTS5 (ı/İ/ş/ğ/ç) — no plugin
>
> Hardest one to get right: the paste path that survives self-capture loop dedup.

---

## Hacker News pre-flight checklist (do these before posting)

- [ ] Repo is public ✓
- [ ] Tag `v0.1.x` published; latest release downloadable
- [ ] README has install instructions visible above the fold
- [ ] At least 2 screenshots / 1 GIF in README (popup, settings, sensitive-content guard)
- [ ] `SECURITY.md` reachable
- [ ] `LICENSE` file present (Apache-2.0)
- [ ] Issues tab open; templates not required v1, but a "Bug report" template welcomes early issues
- [ ] First-time install tested on a clean Windows VM in the last 24 hours — SmartScreen warning visible, install succeeds, popup opens, hotkey works
- [ ] Auto-update tested at least once between two consecutive tags
- [ ] CI is green on `main` (so visitors poking around don't see red)

If any unchecked item is a hard stop (e.g. no screenshots), post anyway only if the absence is visible from the README — half-hearted launches age worse than honest "screenshots coming Friday" notes.

---

## Pre-launch UX traps to avoid

1. **SmartScreen scares first-timers.** Without an EV cert, ~half the users will close the installer. Add a 2-line note in the README's Download section explaining "Click 'More info' → 'Run anyway'" — most readers will skim past anything longer.
2. **Tray icon overflow.** On Windows 11, new apps land in the chevron `▲` overflow by default. The first-run user can't find Klipo. Onboarding step 4 covers this; the README does too. Don't under-explain.
3. **First-run with empty popup.** If a fresh install opens with zero clips, the empty state should still feel responsive. Already the case — popup says "No clips yet — copy something with Ctrl+C and it will appear here."
4. **Performance feels off until Defender stops scanning.** First-time launch on a fresh Windows + Defender real-time scan is slower than steady-state. Don't claim sub-100ms in launch posts; measure with Defender warm. The runbook already accounts for this.
5. **Power users will read the schema migration file.** `001_initial.sql`'s seed list of password manager exe names is functional, not branding — the comment block at the top of `storage/mod.rs` explains the policy. If a contributor PR's a "let's clean these up" change, point them at that comment.
