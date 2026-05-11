# Changelog

All notable changes to Klipo will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.6] — 2026-05-11 — Icon contrast hotfix (white-on-blue squircle)

### Fixed — app icon legibility on dark taskbars
- v0.1.5 shipped the new Klipo K-clipboard glyph as **brand blue on
  transparent background**. On Windows 11's default dark taskbar, the
  blue glyph competed with the dark surface for contrast and read as
  faint / illegible at standard taskbar sizes. User-reported regression.
- v0.1.6 wraps the same K-clipboard glyph in a **solid
  bluedev-blue squircle** (`#015AFF` background, ~22% corner radius
  matching Windows 11 / iOS / macOS adaptive masks) with the glyph
  itself painted **white**. The composition mirrors the well-trodden
  Discord / Slack / VS Code icon pattern: a brand-colored backplate
  guarantees high contrast on every surface (dark/light/themed
  taskbars and docks alike), while the white glyph carries the
  identity. No glyph geometry changed — only the rendering shell.
- `src-tauri/scripts/render-icon.mjs` updated to emit the squircle +
  white glyph composition. Re-run before each release for a refresh.

### Notes
- All `src-tauri/icons/*` PNG/ICO/ICNS/Android/iOS variants regenerated
  with the new composition.
- KLIPO_PRODUCT_ID_DEFAULT still placeholder — license activation
  will be wired in the next release after the Gumroad listing saves
  and the real product_id is known.

---

## [0.1.5] — 2026-05-11 — App icon refresh + brand-strict UI accents

### Changed — app icon
- All `src-tauri/icons/` PNG/ICO/ICNS variants regenerated from
  [`assets/klipo-mark.svg`](./assets/klipo-mark.svg) — the K-clipboard
  combination that mirrors the bluedev B-mark. The previous placeholder
  icon (generated locally in M7) is now replaced with the production
  Klipo mark across Windows tray, executable, NSIS installer, and any
  future macOS/iOS/Android bundle artifacts.

### Fixed — brand-strict UI accents
- Popup "hint" toast (file → browser path-copy notice) was rendered in
  `yellow-500` background + `yellow-200` text — off-brand under the
  bluedev palette (no warm accents allowed; see
  [`shared/marketing-asset-pipeline.md`](../../.claude/skills/gumroad-launch/shared/marketing-asset-pipeline.md)
  in the gumroad-launch skill). Switched to `primary` (brand blue) for
  consistency. Companion to v0.1.3's marketing-side amber sweep.

### Notes
- Settings → License tab's amber pills for "Offline grace" and "Trial
  countdown (≤ 3 days)" intentionally remain — those communicate
  product state ("approaching cutoff") and brand audit treats them as
  carve-outs alongside `clip-row--featured`'s red border for sensitive
  clips. Will be revisited if user feedback flags them.

### Pre-live TODO (still pending)
- Replace `KLIPO_PRODUCT_ID_DEFAULT` in
  [`src-tauri/src/license/mod.rs`](./src-tauri/src/license/mod.rs)
  once the Gumroad listing is published and the real product_id is
  known. Activation calls fail with a friendly error until then.

---

## [0.1.4] — 2026-05-10 — Version-sync hotfix + trial-then-license activation

**Skipping v0.1.3 forward.** The v0.1.3 release shipped with `Cargo.toml`
still pinned at `0.1.2` (only `package.json` and `tauri.conf.json` were
bumped). Tauri's `getVersion()` reads from `Cargo.toml`'s
`CARGO_PKG_VERSION` at compile time, so the v0.1.3 binary self-reported
as `"0.1.2"` while the manifest advertised `"0.1.3"`. The auto-updater
thus saw a same-or-older comparison and refused to offer the upgrade,
leaving installed Klipo instances stuck on `0.1.2`. v0.1.4 corrects all
four version sources (`Cargo.toml`, `package.json`, `tauri.conf.json`,
hardcoded UI strings) so they advance together. v0.1.3 published
artifacts remain on GitHub Releases for the historical record but are
not the recommended download.

### Added — 14-day trial + Gumroad license activation
- **Trial-then-license model.** First launch records `trial_started_at`;
  full features for 14 days. Footer shows `Trial: N days left` countdown
  (yellow when ≤3 days). Trial expired + no license → clipboard capture
  pauses, popup overlays an "Activate license" prompt (history is preserved).
- **Gumroad license activation.** Settings → License: enter email + key,
  Klipo posts to `api.gumroad.com/v2/licenses/verify` with
  `increment_uses_count=true`, persists state, flips to Pro mode. Refund
  / chargeback / disputed → license cleared automatically (refund-loop
  closure). 30-day offline grace from last verify, periodic re-verify
  every 7 days. Mirrors the WA contacts exporter model
  (`license-manager.js`) ported from chrome.storage to Tauri/SQLite.
- **Pre-live TODO:** replace `KLIPO_PRODUCT_ID_DEFAULT` in
  `src-tauri/src/license/mod.rs` once the Gumroad listing is live.
  Activation calls fail with a friendly error until then.
- 13 new unit tests (3 in `gumroad.rs`, 10 in `manager.rs`) — total
  64 → 77, all passing.

### Branding (continued from v0.1.3)
- All v0.1.3 changes carry forward: bluedev publisher metadata, Settings
  → About bluedev links, README hero "by bluedev" badge, honest
  SmartScreen note, $29 Gumroad pricing copy, demo video script +
  shotlist.

---

## [0.1.3] — 2026-05-10 — Commercial pivot + sensitive-content fixes + popup limit + re-scan history

First commercial release under **bluedev** brand. Licensing model
changed from Apache-2.0 to a proprietary EULA owned by Aydoğan Dağıdır
(trading as bluedev). v0.1.0–0.1.2 remain available under Apache-2.0;
v0.1.3+ is governed by [`LEGAL/EULA.md`](./LEGAL/EULA.md). Distributed
on Gumroad at $29 with lifetime v0.x updates.

**Note:** v0.1.3 is **not Authenticode-signed** — EV cert cost was
deliberately deferred. Installer carries `Publisher: bluedev` metadata
visible in NSIS Properties → Details. SmartScreen will show "Unknown
publisher" on first install — see README "Note on SmartScreen" for the
trust-anchor rationale. Auto-update payload is still Ed25519-signed
(unchanged from prior releases).

### Fixed — sensitive content detection
- **OpenAI project keys (`sk-proj-…`)**, service-account keys
  (`sk-svcacct-…`), and admin keys (`sk-admin-…`) are now flagged as
  sensitive. The legacy regex assumed pure-alphanumeric bodies and
  silently let through the post-2024 OpenAI formats, which include
  dashes. Reproduces the user-reported bug where `sk-proj-…` clips
  appeared in the popup without a red border or blurred preview.
  Updated pattern lives at
  [`src-tauri/src/clipboard/sensitive.rs`](./src-tauri/src/clipboard/sensitive.rs)
  and is mirrored in [`docs/security.md` §3.1](./docs/security.md).
- New regression tests cover all four OpenAI key formats and an
  explicit assertion that Anthropic's `sk-ant-…` keys are not
  mis-classified as OpenAI keys.

### Added — Settings → Privacy → Re-scan history
- **One-tap re-scan** of the existing clip history with the current
  sensitive-content regex set. Backed by a new Tauri command
  `resensitize_history` and a `Storage::resensitize_all` method that
  is **strictly UPDATE-only**: only the `sensitive` flag (and
  `sync_version`) ever changes — no clip is inserted, deleted, or
  rewritten. Soft-deleted rows are skipped.
- Returns a `ResensitizeReport { scanned, flagged, unflagged, unchanged }`
  surfaced as a toast in the Settings UI. Solves the v0.1.3 migration
  case: clips captured before the regex bump still carried their old
  verdict; one click brings them in line with the new rules without any
  data loss.
- Covered by 4 storage tests (flip, idempotent, soft-delete-skip, unflag
  on regex loosening) and a manual verification step in dev mode.

### Changed — licensing & branding
- License switched to a proprietary EULA. See [`LICENSE`](./LICENSE),
  [`LEGAL/EULA.md`](./LEGAL/EULA.md), [`LEGAL/PRIVACY.md`](./LEGAL/PRIVACY.md),
  [`LEGAL/REFUND.md`](./LEGAL/REFUND.md). Apache-2.0 text preserved at
  [`LICENSE-Apache-2.0-historical.md`](./LICENSE-Apache-2.0-historical.md).
- README now identifies bluedev as the publisher and points at the
  Gumroad listing (when live).

### Fixed — popup display limit
- Popup was hard-coded to load only the first 50 clips, which made the
  history feel artificially capped even when `history_limit` was set
  to 10,000. Now reads `history_limit` at mount and loads up to
  `min(history_limit, 1000)` per refresh — backend clamp also raised
  to 10,000. The 1,000 popup ceiling stays sane until `react-virtual`
  is wired (planned v0.1.4); search (Ctrl+F) covers anything older.

### Added — pre-launch documentation
- Gumroad product page copy: [`docs/gumroad-product-page.md`](./docs/gumroad-product-page.md) ($29 launch).
- bluedev.dev landing page copy: [`docs/landing-bluedev.md`](./docs/landing-bluedev.md).
- Demo video script + shotlist: [`docs/demo-video-script.md`](./docs/demo-video-script.md), [`docs/demo-video-shotlist.md`](./docs/demo-video-shotlist.md).
- Release-signing walkthrough (Authenticode reference for future):
  [`docs/release-signing.md` §6](./docs/release-signing.md).

### Branding
- `tauri.conf.json` `bundle.publisher = "bluedev"`, `bundle.homepage = "https://bluedev.dev"`.
- README hero, Settings → About, and Settings sidebar footer all mention bluedev.
- Repository moved to **private** (release artifacts remain public for the auto-update endpoint).

---

## [0.1.2] — 2026-05-06 — Relaunch discoverability + release pipeline hardening

This release ships almost no runtime changes; it's the first release proven
to flow end-to-end through `tauri-plugin-updater`, and folds in the
docs / onboarding gaps that surfaced once a real user updated from v0.1.0
to v0.1.1 in production.

### Added — relaunch UX
- **Onboarding step 4** (the tray-quit step) gained a "to use Klipo again
  after quitting" paragraph: `Win` → type `Klipo` → Enter, plus the
  Settings → General → Run-at-login pointer for users who never want to
  think about it.
- **Settings → About → Quit Klipo block** now has a sibling paragraph
  describing the same Start-menu relaunch path, so users who reach for
  the Quit button discover the round-trip in the same view.
- **README → "How to bring Klipo back after quitting"** section added
  next to the existing "How to quit". Lists the three relaunch paths
  (Start menu, `%LOCALAPPDATA%\Klipo\Klipo.exe`, `Win+R klipo`) and
  recommends Run-at-login for daily-driver setups.

### Fixed — release pipeline (lessons from v0.1.0/v0.1.1 cycle)
The chain of issues that blocked v0.1.0/v0.1.1 from publishing a working
auto-update endpoint, all settled in this version:

- `pnpm/action-setup@v4` strict-version conflict with `package.json`'s
  `packageManager` field (`ERR_PNPM_BAD_PM_VERSION`). YAML pin removed.
- Hand-rolled release pipeline didn't generate a manifest.
  Switched to `tauri-apps/tauri-action@v0` for the bundle + draft-release
  step.
- `tauri-action`'s asset filter excluded the updater payloads.
  Replaced its `includeUpdaterJson` path with a dedicated PowerShell
  step that reads `<installer>.sig` and writes a Tauri-shaped
  `latest.json` directly to the release.
- Tauri 2.x defaults `bundle.createUpdaterArtifacts` to `false`, so no
  `.sig` file was being produced. Set it to `true` in `tauri.conf.json`.
- The PowerShell step initially pointed `latest.json`'s `url` at the
  obsolete Tauri 1.x `.nsis.zip` path. Updated to the direct
  `Klipo_<v>_x64-setup.exe` URL — Tauri 2 verifies the installer
  bytes against the embedded signature and runs it silently.
- Repo went public so the `releases/latest/download/...` redirect could
  resolve without an auth header (the updater plugin doesn't ship
  bearer tokens to user binaries).

### Verified — v0.1.2
- ✅ `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
- ✅ `cargo test --lib` — 56 tests pass (no behavior changes since v0.1.1)
- ✅ `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm build`
- ✅ Live end-to-end auto-update from v0.1.0 → v0.1.1 succeeded in
  production: `latest.json` fetched, signature validated, installer
  downloaded, app restarted at the new version.

## [0.1.1] — 2026-05-06 — Quit UX + CI workflow fix

First post-launch hotfix. Two real-world issues from the v0.1.0 install
flow surfaced:

### Fixed
- **CI workflow conflict with `pnpm/action-setup@v4`** — both `version: 9`
  in workflow YAML and `packageManager: pnpm@9.12.0` in `package.json`
  triggered `ERR_PNPM_BAD_PM_VERSION` and aborted both CI and Release
  pipelines in 6 seconds. Removed the redundant YAML pin; `package.json`
  is now the single source of truth, action-setup discovers from it.

### Added — Quit UX (was undiscoverable in v0.1.0)
- **`Ctrl+Q` shortcut** in the popup quits Klipo entirely (vs. `Esc`
  which only hides the popup). Footer now shows the hint.
- **`commands::quit_app` IPC** — calls `app.exit(0)` so plugin / storage
  drop hooks run before the process terminates.
- **Settings → About → "Quit Klipo" button** with explanatory text about
  the Windows tray icon (chevron `▲` overflow on Win 11) and the
  difference between hide-popup and quit-app.
- **Onboarding tour gained a 4th step** ("Klipo lives in your tray")
  that walks new users through the tray right-click → Quit flow + the
  `Ctrl+Q` shortcut. Surfaces both paths so neither is missed.
- **README "How Klipo Works" gained a "How to quit Klipo" section**
  enumerating all three quit paths.

### Verified — v0.1.1
- ✅ `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
- ✅ `cargo test --lib` — **56 tests pass** (no test changes since v0.1.0)
- ✅ `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm build`

## [0.1.0] — 2026-05-06 — Phase B complete: Windows MVP

First public release. Klipo runs as a daily-driver clipboard manager on Windows 10 1809+ / Windows 11 with full feature parity for the Phase B scope: capture (text / image / file / RTF / HTML), dedup, sensitive-content guard, native drag-and-drop, complete Settings UI, hotkey rebind, autostart, and onboarding tour.

### Added — M7 (auto-update plugin, perf instrumentation, version 0.1.0)
- `tauri-plugin-updater` reinstated in `Cargo.toml` + `lib.rs` builder. Default `tauri.conf.json` ships with a placeholder pubkey + GitHub Releases endpoint pattern; until the placeholder is replaced via the `pnpm tauri signer generate` flow described in `docs/release-signing.md`, the "Check for updates" button surfaces a friendly "not configured" message instead of crashing.
- `@tauri-apps/plugin-updater` added to `package.json`. Frontend wraps `check()` + `downloadAndInstall()` behind `checkForUpdates()` / `downloadAndInstallUpdate()` in `src/lib/ipc.ts`.
- Settings → General → "Updates" row: idle / checking / available (with version + release notes) / install / error states. Lazy-imports the plugin so the popup bundle stays lean.
- `src-tauri/src/perf.rs` extended with `POPUP_FIRST_VISIBLE_LOGGED` + `HOTKEY_PRESS_INSTANT` so the popup's window-event listener can stamp `popup_visible_ms` (process-start → first focus) and `hotkey_to_focus_ms` (hotkey press → re-focus). Surfaces under the `klipo::perf` tracing target.
- `src/App.tsx` logs `klipo:search_ms` to DevTools console verbose level on each FTS5 search round-trip — feeds into the §3 measurement in `docs/perf-runbook.md`.
- `docs/release-signing.md` (new): step-by-step walk-through of `pnpm tauri signer generate`, GitHub repo secrets (`TAURI_SIGNING_PRIVATE_KEY` / `..._PASSWORD`), end-to-end verification with one full release cycle, and key rotation procedure.
- Version markers bumped from `0.1.0-dev` → `0.1.0` in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, popup footer, Settings sidebar footer, and About tab.

### Added — M6.5 (Excluded apps capture-foreground, toggle visibility, perf runbook)
- `src/components/ToggleSwitch.tsx` — extracted from `PrivacyTab` / `GeneralTab`, redesigned with `bg-input` track + `border` + thumb `shadow-md ring-1 ring-black/10` so the off-state is visibly distinct in light theme. Also adds a `focus-visible:ring` for keyboard accessibility.
- IPC `capture_foreground_app(delay_ms)` — hides Settings for 1–10 s (default 3 s), snaps `GetForegroundWindow`'s identifier, reopens Settings. Wired into Excluded Apps → Add Form via a `Crosshair` button with a live countdown; auto-fills both the bundle id and a label derived from the exe stem.
- `docs/perf-runbook.md` (new): companion to `perf-budget.md`. Manual end-to-end measurement playbook for cold start, warm hotkey, search latency, paste latency, RAM idle / loaded, bundle size. Provides PowerShell snippets and the recording template for `bench/results-<yyyy-mm>.md`.

### Added — M6.1 (Excluded apps editor)
- `Storage::list_excluded_apps` / `add_excluded_app` / `remove_excluded_app` —
  full CRUD over the `excluded_apps` table; idempotent insert (re-add of an
  existing `bundle_id` updates its label rather than creating a duplicate).
- IPC commands: `list_excluded_apps`, `add_excluded_app`, `remove_excluded_app`.
- `src/routes/settings/ExcludedAppsTab.tsx` — list with per-row remove button,
  inline add form (process name + optional label).
- 2 new storage tests covering insert/update/remove and empty-id rejection.

### Added — M6.2 (Privacy tab + data ops)
- `Storage::wipe_all_clips` — hard-deletes every clip row (FTS5 triggers
  cascade). Settings + `excluded_apps` are intentionally NOT touched.
- IPC commands: `app_data_dir_path`, `open_data_folder`, `wipe_all_data`.
  `wipe_all_data` deletes the `blobs/` and `thumbs/` subtrees under app data
  and surfaces partial failures as readable string errors.
- `src/routes/settings/PrivacyTab.tsx`:
  - Telemetry toggle (default off, persists `telemetry` setting).
  - Sync placeholder with `v0.3` badge — disabled until Faz D ships.
  - Data folder reveal — opens Explorer / Finder at the app data dir.
  - Wipe-all action behind an AlertDialog confirm.
- 1 new storage test verifying wipe preserves settings + exclusions.

### Added — M6.3 (Hotkey rebind)
- `lib::parse_chord` — accepts strings like `Ctrl+Alt+V`, `CmdOrCtrl+Shift+P`,
  `Alt+Shift+5`, `Ctrl+F12`. Modifier names are case-insensitive with
  cross-platform aliases (`CmdOrCtrl`, `Option`, `Win`, etc.). Main key must
  be A–Z, 0–9, or F1–F24.
- `lib::handle_hotkey` (free function) — replaces the inline handler closure
  so startup AND the rebind IPC can register against the same logic.
- `lib::CURRENT_HOTKEY` — `OnceLock<Mutex<Option<Shortcut>>>` tracking the
  active chord so the rebind path can unregister it cleanly.
- IPC command `register_hotkey(chord)` — atomic swap with rollback: if the
  new chord fails to register, the old one is restored before returning the
  error to the UI.
- Startup now reads the saved `hotkey` setting and prefers it over the
  built-in `Ctrl+Alt+V` / `Ctrl+Alt+Shift+V` fallback chain.
- `HotkeyRebindRow` in the General tab — chord-capture input that listens
  on `window keydown` (capture phase), filters out bare modifier presses,
  shows live errors for unsupported keys, and persists on first valid
  combo. 8 new chord-parser unit tests.

### Added — M6.4 (Autostart on login)
- `src-tauri/src/autostart.rs` — Windows registry helpers built on the
  `windows` 0.61 crate: writes / reads / deletes a `Klipo` value under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Per-user (no admin
  needed). macOS path stubbed for v0.2.
- `Cargo.toml` — added `Win32_Security` + `Win32_System_Registry` features
  to the existing `windows` dep.
- IPC commands `get_autostart` / `set_autostart`.
- General tab: "Run at login" toggle with optimistic update + revert on error.

### Added — M7 (onboarding + docs)
- `src/components/OnboardingOverlay.tsx` — 3-step welcome wizard (Welcome →
  Hotkey → Pin/Delete/Search) with skip + per-step indicator. Shows on
  first popup focus when `onboarding_done !== "on"`; persists to settings
  on completion.
- About tab → "Replay onboarding" button: resets the flag so the next
  popup focus shows the tour again.
- `SECURITY.md` (repo root) — vulnerability disclosure policy, scope, SLA.
- `CONTRIBUTING.md` — setup, verification pipeline, commit/PR conventions.
- `README.md` — status bumped to M1–M6 done; cleaned up Quick Start.
- `.github/workflows/release-windows.yml` — full pipeline: build, package,
  upload artifacts, create draft GitHub Release on `v*` tag pushes (with
  auto-generated notes). Code-signing hooks stay placeholder until an EV
  cert is acquired.

### Added — M6.0 (Settings UI skeleton + Theme)
- Second Tauri window `settings` (720×640, decorated, opaque) registered in
  `tauri.conf.json` and added to the default capability allowlist. Distinct
  from the popup `main` so Settings can have normal app chrome while the
  popup stays frameless and Mica/Acrylic-blurred.
- `src/main.tsx` — picks `App` (popup) or `Settings` route based on
  `window=settings` query param, and stamps `data-window="popup"|"settings"`
  on `<html>` so `globals.css` can scope per-window background styles.
- `src/routes/Settings.tsx` — sidebar layout with 4 tabs: General (wired),
  Excluded apps (stub for M6.x), Privacy (stub for M6.y), About (version,
  license, platform, user-agent).
- `src/routes/settings/GeneralTab.tsx`:
  - **Theme picker (light / dark / system)** — applies live, persists to
    SQLite. System mode tracks `prefers-color-scheme` changes via
    `MediaQueryList` listener.
  - **History limit** — number input + Save button, validated 100–1,000,000.
  - **Hotkey display** — read-only for now; rebind UI lands in M6.1.
- `src/lib/theme.ts` — single source of truth for `applyTheme(mode)`,
  `applyThemeFromSetting()` (called once at bootstrap), and `setTheme(mode)`
  (persist + apply).
- `src-tauri/src/commands.rs` — three new IPC commands:
  - `open_settings` — show + focus the Settings window (or surface a clear
    error if the config drift removes it).
  - `get_setting` / `set_setting` — whitelist-gated key/value access to the
    `settings` SQLite table; per-key `validate_setting` rejects bad ranges.
- `src-tauri/src/storage/clips.rs` — `get_setting` / `set_setting` storage
  methods backed by the existing `settings` table from `001_initial.sql`.
- `src-tauri/src/lib.rs`:
  - Tray menu now has a "Settings…" item between "Show Klipo" and "Quit".
  - Settings window listens for `CloseRequested` and hides instead of
    destroying — re-open via tray or popup gear icon stays instant.
- `src/App.tsx` — gear icon next to the clip count in the popup header
  opens the Settings window via `openSettings()` IPC.

### Verified — Phase B v0.1.0
- ✅ `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
- ✅ `cargo test --lib` — **56 tests pass** (was 45 at M6.0; +11 across
  settings, excluded-apps, wipe, chord parser).
- ✅ `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm build`.

## [0.0.11-m5.x.1] — 2026-05-05 — Phase B M5.x.1: Native drag-and-drop

### Added — M5.x.1
- `tauri-plugin-drag` (CrabNebula) integration so file/image clips can be
  dragged out of the popup into a target window. Chromium-based apps
  (Discord, Slack, Notion, browser shells) reject `Ctrl+V` of file payloads
  for security; native OS drag is the standard accepted path.
- `src/lib/drag.ts` — `startNativeDrag(clip)` resolves blob paths, calls
  `@crabnebula/tauri-plugin-drag::startDrag`. File clips drag the JSON path
  list straight from `text_content`; image clips drag the absolute PNG blob.
- `src/components/ClipCard.tsx`:
  - Window-level `mousemove` + `mouseup` listeners detect drag intent via
    a 3-px threshold, surviving cursor leaving the popup.
  - GripVertical handle (`⋮⋮`) starts an immediate drag on mousedown — no
    threshold — for users who prefer an explicit handle.
  - `<img>` thumbnails get `draggable={false}` + `onDragStart preventDefault`
    so the browser's HTML5 drag doesn't suppress our window-level handlers.
- `src-tauri/capabilities/default.json` — `drag:default` permission added so
  the plugin's `start_drag` command actually runs (silent reject without it).
- `src-tauri/src/clipboard/paste.rs` — file paste now writes both CF_HDROP
  (Explorer / Outlook accept) and CF_UNICODETEXT (browser fallback that
  shows the file path so the user can manually upload it).
- `src/App.tsx` — file paste into a Chromium-based app shows a 6-second
  inline hint banner explaining why upload didn't happen.

### Verified — M5.x + M5.x.1
- ✅ Pin / unpin via icon click + Ctrl+P
- ✅ Delete clip via Backspace / Delete (when search not focused)
- ✅ Sensitive paste confirm via AlertDialog
- ✅ Last-active-app indicator (`→ exe.name`) chip
- ✅ Hotkey fallback chain (Ctrl+Alt+V → Ctrl+Alt+Shift+V → tray-only)
- ✅ Türkçe-Q AltGr layout detection log warning
- ✅ Tray icon (Show / Quit menu, left-click toggle)
- ✅ Image drag → Paint, Word, browser drop zones
- ✅ File drag → Explorer / Outlook (CF_HDROP) and browser drop zones (native)

### De-Anthropic'd
- Removed all "Claude" / "Anthropic" references from product-facing surfaces:
  - `CLAUDE.md` → renamed to `AGENT.md`, content scrubbed of Claude Code
    references.
  - `README.md`, `CHANGELOG.md`, `App.tsx`, `drag.ts`, `paste.rs`,
    `Cargo.toml` comments — third-party app references generalized to
    "Chromium-based apps" / "browser shells / Discord / Slack / Notion".
  - `isBrowserLikeApp()` heuristic list dropped "claude" — `chrome` /
    `msedge` already match the Chromium browser shells regardless of brand.
- Kept legitimate technical detection: `anthropic_key` regex pattern in
  `sensitive.rs` stays (sensitive content auto-detection of Anthropic API
  keys, alongside OpenAI / Stripe / GitHub patterns — protects user keys,
  not a development-origin signal).

## [0.0.9-m5.x] — 2026-05-05 — Phase B M5.x: Mini polish

## [0.0.10-m3.2] — 2026-05-05 — Phase B M3.2: Multi-format capture & paste

### Added — M3.2

#### Storage layer
- `src-tauri/src/storage/blob.rs` (new):
  - `Storage::blob_root` / `thumb_root` / `resolve_blob` / `resolve_thumb`
    helpers that resolve relative `<sha[:2]>/<sha>.<ext>` paths against the
    DB's parent directory (per-user `%APPDATA%\app.klipo.desktop\blobs\…`).
  - `write_blob` (idempotent, 50 MB hard cap) and `write_thumbnail` (192-px
    long-edge WebP, lazy via `tokio::task::spawn_blocking`).
  - `reencode_to_png` strips EXIF metadata + normalizes the SHA-256 hash so
    `Win+Shift+S` and Snipping Tool screenshots dedup properly.
- `Storage::db_dir()` — exposes the DB's parent directory for blob path
  resolution. `None` for `open_in_memory()`.
- 8 new unit tests (39 → 41 total): blob path sharding, idempotent writes,
  oversize rejection, PNG round-trip, db_dir resolution, in-memory None.

#### Clipboard subsystem
- `src-tauri/src/clipboard/normalize.rs` (new): Win32 format readers under
  one roof. Each function takes the clipboard already-open per
  `# Safety` doc:
  - `read_unicode_text` → `String`
  - `read_html` → full CF_HTML payload (Microsoft header + body)
  - `read_rtf` → CF_RTF lossy-decoded
  - `read_file_paths` → `Vec<String>` via `DragQueryFileW`
  - `read_image_as_png` → re-encoded PNG bytes + SHA-256 (CF_DIBV5 → CF_DIB
    → DIB-to-BMP wrap → image crate decode → PNG re-encode)
- `src-tauri/src/clipboard/watcher_windows.rs` (rewritten capture path):
  format priority resolver — file > image > html > rtf > text — runs once
  per `WM_CLIPBOARDUPDATE`, builds the appropriate `ClipboardEvent`.
- `src-tauri/src/clipboard/pipeline.rs`: now writes binary blobs
  (`write_blob`) before inserting the row, schedules background thumbnails,
  and stores `text_content=None` for image clips.
- `src-tauri/src/clipboard/paste.rs` (rewritten): kind-aware paste:
  - **text** → `CF_UNICODETEXT`
  - **html** → `CF_HTML` (registered format) + plain-text fallback
  - **rtf**  → `CF_RTF` (registered format) + plain-text fallback
  - **file** → `CF_HDROP` (DROPFILES struct + concatenated wide-string paths)
  - **image** → `CF_DIB` (PNG → BMP → strip 14-byte file header)
  Then `SendInput(Ctrl+V)`. `strip_html_tags` and `strip_rtf` keep paste
  reasonable when the target app doesn't speak the rich format.

#### IPC + frontend
- `commands::paste_clip` now takes the full `Clip`, resolves `blob_root`,
  and dispatches to `paste::paste_clip` for any kind.
- `commands::resolve_blob_path` and `commands::resolve_thumb_path` —
  return absolute paths the frontend pipes through `convertFileSrc`.
- `tauri.conf.json` enables `assetProtocol` with scope
  `["$APPDATA/blobs/**", "$APPDATA/thumbs/**"]`; CSP `img-src` extended to
  allow `asset:` and `http(s)://asset.localhost`.
- `Cargo.toml` `tauri = { features = ["protocol-asset"] }`.
- `src/lib/ipc.ts`: `blobAssetUrl(relative)` and `thumbAssetUrl(hash)` —
  resolve via IPC then `convertFileSrc`.
- `src/components/ClipCard.tsx`: image clips render a 32-px thumbnail
  (lazy `useImageThumb` hook); file clips show the first filename + count.

#### Documentation
- `docs/m3.2-test-corpus.md` — 10-row manual test matrix (text, multi-line,
  URL, Türkçe, RTF, HTML, PNG, JPEG, single file, multi-file) plus edge
  cases (sensitive RTF, oversize image, Türkçe paths, empty payload, self-
  paste loop).

### Changed — M3.2
- `Cargo.toml` MSRV bumped 1.75 → **1.83** (uses `std::io::ErrorKind::FileTooLarge`
  for blob-cap rejection; stable since 1.83). User toolchain is 1.95 — no
  practical impact.
- `Cargo.toml` adds `image 0.25` (PNG/JPEG/BMP/WebP features only,
  default-features off — keeps bundle compact).

### Verified — M3.2 pipeline
- [x] `cargo check --all-targets` clean (37s).
- [x] `cargo clippy --all-targets -- -D warnings` clean (23s).
- [x] `cargo fmt --check` clean.
- [x] `cargo test --lib` 41/41 pass (0.07s).
- [x] Frontend pipeline: typecheck / lint / format:check / build clean
      (174.97 KB JS / gzip 56.65 KB / 10.04 KB CSS).

### Manually verified (2026-05-05)
- ✅ All 10 rows of `docs/m3.2-test-corpus.md` (text, multi-line, URL,
  Türkçe + dotless-i search, RTF→OneNote bold, HTML→Word, PNG screenshot,
  JPEG from browser, single file, multi-file).
- ✅ Image paste cross-app: Paint, Chromium-based web app, Outlook all accept the same
  clip — `arboard` (CF_BITMAP+CF_DIB) + additive CF_PNG covers Win32 +
  Chromium-based + Office.

### Fix during manual test (2026-05-05)
- **Image paste fail in Chromium-based browsers:** arboard alone wrote
  CF_BITMAP+CF_DIB but Chromium needs `CF_PNG` (custom registered format).
  Added `add_png_format` that re-opens the clipboard *additively* (no
  EmptyClipboard) and writes the raw PNG bytes under
  `RegisterClipboardFormatW("PNG")`.
- **Race with our own watcher:** Klipo's WM_CLIPBOARDUPDATE listener
  raced with the paste path's `OpenClipboard`. Added
  `WATCHER_PAUSED: AtomicBool` + RAII `WatcherPauseGuard` so the watcher
  skips capture during paste (~200 ms window).
- **SendInput hitting wrong window:** `arboard::set_image()` briefly
  switches foreground to its internal owner window. Added a second
  `SetForegroundWindow(prev_hwnd)` + 15 ms sleep right before SendInput.
- **HTML clip preview showing "Version:1.0":** popup was rendering the
  Microsoft CF_HTML header. Added `htmlPreview()` in `ClipCard.tsx` that
  extracts the body between `<!--StartFragment-->...<!--EndFragment-->`,
  strips tags, decodes common entities. RTF preview gets the same
  treatment (`rtfPreview`).
- **Türkçe FTS search miss (`isik` → `ışık`):** SQLite `unicode61
  remove_diacritics 2` doesn't fold the dotless-i family. Migration 002
  rebuilds the FTS5 trigger with a 12-char REPLACE chain
  (ı/İ/ş/Ş/ğ/Ğ/ü/Ü/ö/Ö/ç/Ç → ASCII), and Rust `search::turkish_fold`
  applies the same fold to the query.

## [0.0.8-m3.2] — 2026-05-05 — Phase B M3.2: Multi-format capture & paste — DONE

## [0.0.7-m5] — 2026-05-05 — Phase B M5: Paste action + search wiring

### Added — M5
- `src-tauri/src/clipboard/paste.rs` — native paste-out: `OpenClipboard` → `EmptyClipboard` → `GlobalAlloc(GMEM_MOVEABLE)` → `GlobalLock` → UTF-16 copy → `GlobalUnlock` → `SetClipboardData(CF_UNICODETEXT)` → `SendInput(Ctrl+V down, V up, Ctrl up)`. 80ms delay before SendInput lets the OS re-foreground the previously active app. Win32 calls run on `tokio::task::spawn_blocking` so the reactor doesn't stall on clipboard locks held by other apps.
- `paste_clip(id)` IPC command — looks up the clip's text, hides the popup, then delegates to `paste::paste_text`. Returns a clear error for image/file payloads (deferred to M5.x).
- `hide_popup` already in M4; reused here for `Esc` handling and the paste flow.
- Frontend `pasteClip(id)` + `hidePopup()` IPC wrappers in `src/lib/ipc.ts`.
- `src/App.tsx`:
  - Search box wired to `searchClips` IPC with a 50 ms debounce; empty query falls back to `listClips` (recency).
  - `Enter` (or click) on a row calls `pasteClip(id)`. UI shows a transient "pasting…" indicator in the footer.
  - `Esc` calls `hide_popup`.
- `Cargo.toml` — `Win32_UI_Input_KeyboardAndMouse` feature on the `windows` crate.

### Verified — M5 pipeline
- [x] `cargo check --all-targets` clean (44s).
- [x] `cargo clippy --all-targets -- -D warnings` clean (18s).
- [x] `cargo fmt --check` clean.
- [x] `cargo test --lib` 31/31 pass (0.04s).
- [x] Frontend pipeline: typecheck / lint / format:check / build clean (174 KB JS / gzip 56 KB).

### Pending manual verification (M4 + M5 together)
- [ ] App starts headless.
- [ ] `Ctrl+Alt+V` → popup with Mica/Acrylic blur.
- [ ] Type into search box → list filters live (FTS5 BM25-ranked).
- [ ] ↑/↓ navigates; selected row highlighted.
- [ ] `Enter` on a row → popup hides → ~80 ms later the previously focused app receives a paste.
- [ ] Repeated paste of the same clip → watcher captures our own write but dedup bumps the row to the top instead of duplicating.
- [ ] `Esc` hides the popup.
- [ ] Captures during popup-open update the list live.

### Deferred
- M5.x: image / file / RTF / HTML paste paths (today text-only; image clip paste returns a typed error).
- M6: Settings UI — hotkey rebind, excluded apps add/remove, theme switcher, history limit, JSON export/import.

## [0.0.6-m4] — 2026-05-04 — Phase B M4: Hotkey + Popup Window

### Added — M4
- `Cargo.toml` — `window-vibrancy 0.6` for Mica (Win 11) / Acrylic (Win 10) blur.
- `tauri.conf.json` — main window now starts **hidden**, frameless (`decorations: false`), transparent, always-on-top, `skipTaskbar: true`. The popup is summoned by hotkey, not launched as a normal app window.
- `src-tauri/src/lib.rs`:
  - `apply_window_blur()` tries Mica first, falls back to Acrylic with a dark tint, no-op on non-Windows.
  - Global hotkey `Ctrl+Alt+V` registered via `tauri-plugin-global-shortcut`. Pressing it toggles popup visibility (show + focus on first press, hide on second).
  - `Focused(false)` window event hides the popup (Spotlight-style auto-dismiss).
- `src-tauri/src/commands.rs` — new `hide_popup` command for the frontend's Esc handler.
- `src/components/ClipCard.tsx` — compact clip row: kind icon (lucide), preview line (truncated to 80 chars), meta row (time-ago, size, source app, sensitive badge), pinned indicator. Sensitive clips render with red left border + content blurred until focus/hover.
- `src/App.tsx` — popup root with search box (UI-only for now; FTS5 wiring lands in M5), keyboard-navigable list (↑/↓), Esc → `hide_popup`, footer hint bar. Auto-focuses search on each show event and keeps the selected card scrolled into view.
- `src/styles/globals.css` — popup-specific transparent background + slim webkit scrollbars.

### Verified — M4 pipeline
- [x] `cargo clippy --all-targets -- -D warnings` clean (9.5s).
- [x] `cargo fmt --check` clean.
- [x] `cargo check --all-targets` clean.
- [x] Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm format:check` clean.
- [x] `pnpm build` 1584 modules, 173 KB JS / gzip 56 KB / 9.3 KB CSS.

### Pending manual verification (next sit-down with the app)
- [ ] App launches headless — no visible window on first run.
- [ ] `Ctrl+Alt+V` shows the popup, focuses the search input.
- [ ] Mica/Acrylic blur visible behind popup (Win 11 → Mica; Win 10 → Acrylic).
- [ ] ↑/↓ navigates the list, selection scrolls into view.
- [ ] Clicking outside the popup hides it.
- [ ] Pressing `Ctrl+Alt+V` again toggles it.
- [ ] `Esc` hides it via `hide_popup` IPC.
- [ ] Captures from M3.1 still flow in (live `clip:new` event refreshes the visible list).

### Deferred to M5 / M6
- M5: actual paste action on Enter (Windows `SendInput` + clipboard write + window hide; preserves user's previous foreground app).
- M5: search bar wires into `search_clips` IPC.
- M6: hotkey rebind UI + AltGr layout detection in settings.

## [0.0.5-m3.1] — 2026-05-04 — Phase B M3.1: Native clipboard watcher (text capture)

### Added — M3.1
- `src-tauri/src/clipboard/watcher_windows.rs` — dedicated worker thread that registers a `KlipoClipboardWatcher` window class, creates a hidden `HWND_MESSAGE` window, calls `AddClipboardFormatListener`, and pumps Windows messages until the process exits. `WM_CLIPBOARDUPDATE` reads `CF_UNICODETEXT` via `OpenClipboard` / `GetClipboardData` / `GlobalLock`, with strict `// SAFETY:` comments on every unsafe op (CLAUDE.md non-negotiable #3). Single-instance via global `OnceLock<UnboundedSender>`.
- `src-tauri/src/clipboard/pipeline.rs` — Tokio task: excluded-app filter → sensitive scan → SHA-256 hash → `Storage::insert_clip` → emit `clip:new` / `clip:bumped`. Logs id + 12-char hash prefix only (never content).
- `src-tauri/src/storage/clips.rs` — `is_app_excluded()` method + test against the seed list.
- `src-tauri/src/lib.rs` — Setup hook now spawns the OS watcher and pipeline after migrations run.
- `src/App.tsx` — subscribes to `clip:new` and `clip:bumped` Tauri events; live `clipCount` refresh + last-event display.

### Changed — M3.1
- `windows` crate bumped 0.58 → **0.61** to unify with Tauri 2.11's transitive `windows-core 0.61` and resolve duplicate-version trait-impl conflicts.

### Verified — M3.1 (on user's machine)
- [x] Native watcher thread spawned, `AddClipboardFormatListener` attached on real HWND.
- [x] Four real `Ctrl+C` captures recorded with correct sizes (112, 183, 4205, 21 bytes).
- [x] Hash-based dedup: same text re-copied → "bumped existing clip" path triggered (4 occurrences).
- [x] AWS access key pattern detected on two distinct clips, both flagged `sensitive=1`.
- [x] Frontend event listener live; UI shows `clips in DB: 4` + `last event: bumped 019df422`.
- [x] `cargo clippy --all-targets -- -D warnings` clean (5.92s).
- [x] `cargo test --lib` 31/31 pass (0.05s).
- [x] `cargo fmt --check` + frontend pipeline (typecheck/lint/format/build) clean.

### Deferred to M3.2
- Image / file path / RTF / HTML format capture (currently text-only).
- Blob disk layout (`%APPDATA%\Klipo\blobs\<sha[:2]>\<sha>.<ext>`).
- 10-format manual test corpus.

## [0.0.4-m3.0] — 2026-05-04 — Phase B M3.0: Clipboard subsystem foundations

### Added — M3.0
- `src-tauri/src/clipboard/mod.rs` — `ClipboardEvent`, `CapturedKind` types; module entry for the watcher subsystem.
- `src-tauri/src/clipboard/sensitive.rs` — 13-pattern `RegexSet` (credit cards, AWS / OpenAI / Anthropic / GitHub / Google / Stripe keys, JWT, PEM/SSH private key headers, URLs with `?token=`, password-field labels). Pattern set is fixed at startup, scanned with a single `RegexSet::matches` call. 13 unit tests cover each pattern's positive case + a benign-text negative case.
- `src-tauri/src/clipboard/source_app.rs` — Windows-only `current()` using `GetForegroundWindow` + `GetWindowTextW` + `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`. Returns `(identifier: "chrome.exe", window_title: "...")`. Non-Windows builds compile to a `None` stub — full impl in v0.2 (Phase C).
- `Cargo.toml` — `regex 1.11`, plus a `[target.'cfg(windows)'.dependencies]` block pulling `windows 0.58` with the Win32 modules needed for the watcher (`Foundation`, `DataExchange`, `Memory`, `Threading`, `ProcessStatus`, `WindowsAndMessaging`, `Graphics_Gdi`).

### Verification (M3.0)
- [x] 29 library tests pass (14 storage + 13 sensitive + 2 source_app/misc).
- [x] `cargo clippy --all-targets --all-features -- -D warnings` clean.
- [x] `cargo fmt --check` clean.
- [x] Frontend pipeline (typecheck/lint/format:check) clean.

### Deferred to M3.1
- Native message pump (`AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE`).
- mpsc channel from native thread → tokio task → `Storage::insert_clip`.
- First end-to-end manual test: `Ctrl+C` something → DB row appears.

## [0.0.3-m2] — 2026-05-04 — Phase B M2: Storage Layer

### Added — M2
- `src-tauri/src/storage/migrations/001_initial.sql` — full v0.1 schema per `docs/storage.md` (clips, FTS5 virtual table + triggers, excluded_apps, settings).
- `src-tauri/src/storage/mod.rs` — `Storage` handle (Arc-cloneable), `open()` / `open_in_memory()`, embedded `sqlx::migrate!` runner, WAL + synchronous=NORMAL pragmas.
- `src-tauri/src/storage/clips.rs` — `Clip`, `ClipKind`, `NewClip`, `InsertOutcome`. CRUD: `insert_clip` (hash-based dedup with `Bumped` outcome), `get_clip`, `list_clips` (pinned-first), `pin_clip`, `soft_delete`, `count_live`.
- `src-tauri/src/storage/search.rs` — FTS5 BM25 search; user query is sanitized into AND-of-prefix tokens to neutralize `*`/`OR`/`NEAR` operator hijacking.
- `src-tauri/src/storage/error.rs` — `StorageError` (sqlx, migrate, NotFound, InvalidKind, Io) via `thiserror`.
- `src-tauri/tests/storage_e2e.rs` — on-disk integration test covering insert/dedup/list/pin/search/soft-delete lifecycle.
- New IPC commands: `list_clips`, `search_clips`, `get_clip`, `pin_clip`, `delete_clip`, `count_live_clips`.
- Frontend `Clip` / `SearchHit` types + wrapper functions in `src/lib/ipc.ts`.
- `src/App.tsx` smoke test now reports both `ping` latency and live clip count.

### Verification (M2)
- [x] 14 unit tests + 1 integration test pass (`cargo test`, ~0.05s + 0.06s).
- [x] `cargo clippy --all-targets --all-features -- -D warnings` clean.
- [x] `cargo fmt --check` clean.
- [x] `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm test`, `pnpm build` clean.
- [x] FTS5 Turkish diacritic-strip path verified (`Merhaba dünya` matched by `dunya`).

### Deferred — pushed to M7
- `tauri-plugin-updater` removed from M1 wiring. The plugin's runtime config requires a `pubkey` (signing public key) which we don't have until release infrastructure exists. M7 (release polish) regenerates a keypair via `pnpm tauri signer generate`, sets up the GitHub Releases endpoint, and re-adds the plugin. Until then auto-update is unavailable in dev builds — acceptable.

## [0.0.2-m1] — 2026-05-04 — Phase B M1: Skeleton

### Added — M1
- `package.json` with pnpm scripts (`dev`, `tauri:dev`, `lint`, `typecheck`, `test`, `format`).
- Vite + React 18 + TypeScript 5 strict-mode frontend skeleton (`src/main.tsx`, `src/App.tsx`).
- Tailwind 3 + shadcn/ui CSS variable theme (light/dark via `.dark` class).
- Type-safe Tauri IPC wrapper (`src/lib/ipc.ts`), single `ping` command for smoke test.
- `src-tauri/` Rust crate: `klipo_lib` library + thin `main.rs` shim, Tauri 2 best-practice layout.
- Tauri 2 `tauri.conf.json` with strict CSP, identifier `app.klipo.desktop`, 480×600 main window.
- Tauri 2 capability file (`src-tauri/capabilities/default.json`).
- Plugins registered: `tauri-plugin-global-shortcut`, `tauri-plugin-updater` (updater inactive until M7).
- `tracing` + `tracing-subscriber` for structured logs (no clipboard content ever logged — see CLAUDE.md non-negotiable #1).
- `.eslintrc.cjs` enforcing `no-explicit-any` (TypeScript non-negotiable #4).
- `.prettierrc` + `prettier-plugin-tailwindcss`.
- `rustfmt.toml` + cargo config that promotes warnings to errors.
- GitHub Actions CI: frontend lint+typecheck+test, Rust fmt+clippy+test, bench compile-only check.
- GitHub Actions release stub for Windows (artifacts unsigned until M7; EV cert later).

### Verification (M1)
- [ ] `pnpm install` succeeds with no peer-dep warnings.
- [ ] `pnpm tauri dev` opens a 480×600 window showing "Klipo" + IPC ping latency.
- [ ] CI green on PR (frontend job + Rust job + bench-compiles job).

## [0.0.1-arch] — 2026-05-04 — Phase A: Architecture

### Added — Phase A
- Repository scaffolding: `README.md`, `LICENSE` (Apache-2.0), `NOTICE`, `CLAUDE.md`.
- `docs/sync-protocol.md` — E2E sync protocol specification (CRDT + HLC).
- `docs/crypto.md` — Cryptographic envelope spec (Argon2id + X25519 + XChaCha20-Poly1305).
- `docs/perf-budget.md` — Performance targets, measurement methodology, hot path analysis.
- `docs/security.md` — STRIDE threat model and mitigations.
- `docs/storage.md` — Local data lifecycle, retention, blob layout, migration plan.
- `docs/hotkey-research.md` — Default hotkey conflict analysis on Windows.
- `bench/` — Criterion-based prototype benchmarks for SQLite + FTS5.

### Decided — Phase A
- Brand renamed from "ClipFlow" → "Klipo".
- License: Apache-2.0 for client; AGPL deferred decision for sync server.
- Platform priority: Windows-first (v0.1) → macOS (v0.2).
- Default hotkey: `Ctrl+Alt+V` on Windows (avoids conflict with paste-without-formatting).
- Crypto: libsodium (sodiumoxide) — no OpenSSL.
- Sync CRDT: LWW-Element-Set with tombstones, ordered by Hybrid Logical Clock.

[0.1.2]: https://github.com/aydogandagidir/klipo/releases/tag/v0.1.2
[0.1.1]: https://github.com/aydogandagidir/klipo/releases/tag/v0.1.1
[0.1.0]: https://github.com/aydogandagidir/klipo/releases/tag/v0.1.0
[Unreleased]: https://github.com/aydogandagidir/klipo/compare/v0.1.2...HEAD
