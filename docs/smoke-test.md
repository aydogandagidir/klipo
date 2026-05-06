# v0.1.0 Smoke Test Checklist

Run this on a **clean Windows VM** (Hyper-V / VirtualBox / Parallels — any) before tagging `v0.1.0`. If any item fails ❌, do not tag — open an issue, fix, retest from §1.

> The clean-VM requirement matters: the dev box has Klipo's old `klipo.db`, registry entries, blob caches, and Defender exclusions that mask real first-run problems.

---

## §1 — VM setup

- [ ] Windows 11 23H2 or later (Pro / Home / Enterprise all OK)
- [ ] WebView2 Evergreen runtime installed (preinstalled on Win 11; install manually for Win 10 1809+ from <https://developer.microsoft.com/microsoft-edge/webview2/>)
- [ ] Defender real-time protection ON (we want to see how a real new install looks)
- [ ] No Klipo footprint anywhere: `%APPDATA%\app.klipo.desktop\` does not exist; `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Klipo` does not exist

Copy the freshly built MSI **and** NSIS installer onto the VM. Do not network-fetch them — bandwidth is unrelated to what we're testing.

---

## §2 — Install

Run the MSI (right-click → Install). Confirm:

- [ ] No SmartScreen "blocked" wall (only the "Unknown publisher" warning, expected until EV cert lands; click "More info" → "Run anyway")
- [ ] Installer completes without prompting for admin (we ship per-user MSI)
- [ ] After install, Klipo launches automatically OR appears in Start menu under "Klipo"
- [ ] Tray icon visible
- [ ] Repeat with the NSIS installer in a fresh VM snapshot — same expectations

---

## §3 — First-run onboarding

- [ ] Press the default hotkey (`Ctrl+Alt+V`)
- [ ] Popup overlay shows the 3-step welcome wizard
- [ ] Step 1 (Welcome) — title, body readable
- [ ] "Next" → Step 2 (Hotkey) — body shows `Ctrl+Alt+V` exact chord
- [ ] "Next" → Step 3 (Pin / Delete / Search) — body readable
- [ ] "Done" closes the wizard and shows the empty popup body
- [ ] Open Klipo Settings → About → "Replay onboarding" → confirms "Done" message
- [ ] Press hotkey again → wizard appears again
- [ ] "Skip tour" works the same as "Done" (persists `onboarding_done = on`)

---

## §4 — Capture (text / image / file / HTML / RTF)

Open Notepad in front of Klipo's popup target. Copy each of the following and confirm a new clip appears in the popup within 500 ms:

- [ ] Plain text from Notepad
- [ ] Multi-line text from Notepad
- [ ] URL from address bar
- [ ] Türkçe karakterli metin (`ışık`, `İğne`, `şarkı`)
- [ ] Excel cell range (RTF + HTML formatlı)
- [ ] Browser-rendered HTML selection (right-click → Copy)
- [ ] PNG screenshot via `Win+Shift+S` → "Copy"
- [ ] JPEG image from a browser (right-click → Copy image)
- [ ] Single file via Explorer (right-click → Copy)
- [ ] Multiple files (3+) via Explorer

For each, confirm:
- [ ] The popup row's icon matches the kind (Type / Image / File / FileText / ScrollText)
- [ ] Image clips show a visible 32 px thumbnail in the row (not a generic icon)
- [ ] Clip's `source_app` chip in the popup header reads correctly (`→ Notepad`, `→ chrome`, etc.)

---

## §5 — Sensitive content guard

- [ ] Copy a credit-card-shaped string `4111 1111 1111 1111` → row shows red left border
- [ ] Copy a string starting with `sk-ant-api03-...` (any random gibberish after) → red border
- [ ] Press `Enter` on a sensitive clip → AlertDialog appears: "Paste sensitive content?"
- [ ] "Cancel" closes the dialog and does NOT paste
- [ ] "Paste anyway" actually pastes into Notepad

---

## §6 — Excluded apps

- [ ] Open Klipo Settings → Excluded apps tab
- [ ] List shows the seeded password manager identifiers (≥8 entries)
- [ ] Click "Capture foreground app" → Settings hides
- [ ] Within 3 seconds, click into Notepad (so it becomes foreground)
- [ ] Settings reopens and the bundle id input contains `notepad.exe`
- [ ] Click "Add" — Notepad shows up in the list
- [ ] Now copy text from Notepad → confirm NO new clip appears in the popup (capture is silently dropped)
- [ ] Remove `notepad.exe` from the excluded list — capture from Notepad resumes on the next copy

---

## §7 — Hotkey rebind

- [ ] Open Settings → General → Hotkey row → "Rebind"
- [ ] Press `Ctrl+Alt+J` → kbd label updates to `Ctrl+Alt+J`
- [ ] Close Settings and press `Ctrl+Alt+J` → popup opens
- [ ] Old hotkey `Ctrl+Alt+V` no longer works
- [ ] Restart Klipo (right-click tray → Quit, then relaunch) → `Ctrl+Alt+J` still works (persisted)
- [ ] Rebind back to `Ctrl+Alt+V` for the rest of the test

---

## §8 — Theme

- [ ] Settings → General → Theme → "Light": entire UI flips to light scheme; toggles still visibly off (grey track, white thumb, dark border) — no invisible controls
- [ ] "Dark": entire UI flips back to dark
- [ ] "System": respects OS theme; flip Windows Settings → Personalization → Colors → Mode and confirm Klipo follows
- [ ] Restart Klipo → last selected theme persists

---

## §9 — Wipe-all + data folder

- [ ] Capture ~10 clips (mix of text + images)
- [ ] Settings → Privacy → "Open data folder" → Explorer shows `%APPDATA%\app.klipo.desktop\` with `klipo`, `klipo.db-shm`, `klipo.db-wal`, `blobs/`, `thumbs/`
- [ ] `blobs/` contains 2-character subdirs with `.png` files; `thumbs/` contains `.webp` files
- [ ] Settings → Privacy → "Wipe all clips & blobs" → confirm dialog "Wipe everything?"
- [ ] "Wipe everything" → toast "Wiped N clip(s)."
- [ ] Reopen popup → empty
- [ ] Inspect data folder again: `klipo` DB still exists (settings + excluded_apps preserved); `blobs/` and `thumbs/` directories are gone

---

## §10 — Autostart

- [ ] Settings → General → "Run at login" → toggle ON
- [ ] Open Registry Editor → `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` → confirm a `Klipo` value with the absolute exe path (in quotes)
- [ ] Reboot the VM
- [ ] After login, Klipo tray icon appears within 30 seconds
- [ ] Toggle OFF → registry value disappears
- [ ] Reboot again → Klipo does NOT autostart

---

## §11 — Updates check (placeholder pubkey path)

- [ ] Settings → General → Updates row → "Check for updates"
- [ ] Either:
  - "Updates not configured for this build (signing key not yet provisioned)." (expected for the placeholder pubkey shipped in the public repo)
  - OR a normal "You're on the latest version" / "Update available" message (only after `docs/release-signing.md` has been followed and a real keypair is in place)
- [ ] No app crash, no error dialog from Tauri itself

---

## §12 — Build pipeline + bundle inspection

On the dev box (not the VM), run:

```powershell
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
cd ..
pnpm typecheck
pnpm lint
pnpm format:check
pnpm build
pnpm tauri build
```

- [ ] All commands exit 0
- [ ] `src-tauri/target/release/bundle/msi/Klipo_0.1.0_x64_en-US.msi` exists, <15 MB
- [ ] `src-tauri/target/release/bundle/nsis/Klipo_0.1.0_x64-setup.exe` exists, <15 MB
- [ ] `src-tauri/target/release/bundle/.../latest.json` exists if signing keypair env vars were set; otherwise absent (expected for unsigned local builds)

---

## DOD

All 12 sections fully ticked → **safe to tag**:

```bash
git add -A
git commit -m "chore(release): v0.1.0"
git tag v0.1.0
git push --tags
```

GitHub Actions `release-windows.yml` runs automatically. Once it produces the draft Release, run **§1–§3 + §11** one more time against the freshly-downloaded MSI to make sure the signed-and-uploaded artifact behaves the same as the local one. Only then click "Publish release" in the GitHub UI.

If anything regresses between local build and the CI artifact, hold the publish — that's exactly what the draft state is for.
