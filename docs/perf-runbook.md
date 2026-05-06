# Klipo Performance Runbook

> Companion to [`perf-budget.md`](./perf-budget.md). The budget tells you the
> targets; this runbook tells you **how to measure them on a real machine
> before a release**, and where to record the numbers.

The bench crate (`bench/`) covers the SQLite kernel. This runbook covers
everything between the user's keypress and the pixel landing on screen —
the parts criterion can't see.

---

## When to run

- Before tagging any `v*` release.
- When a PR touches the popup show path, the paste path, or the watcher
  pipeline.
- On a quarterly cadence even without code changes (Defender / WebView2 /
  Windows updates can move the floor by 30 ms+).

## Environment to record

| Field           | Why it matters                                                |
| --------------- | ------------------------------------------------------------- |
| OS + build      | Windows 11 23H2 vs Windows 10 21H2 differs noticeably         |
| CPU             | M-series Macs vs Ryzen vs Intel mobile all differ             |
| RAM, SSD class  | Cold start hits SSD; tail latencies hit RAM pressure          |
| Defender state  | Real-time scan adds ~50–150 ms to first launch                |
| WebView2 build  | `Get-AppxPackage Microsoft.WebView2`                          |
| Build profile   | Always **release** for perf measurements (debug is unfair)    |

## What to measure

| # | Metric                                | Target (p95) | How                                          |
| - | ------------------------------------- | ------------ | -------------------------------------------- |
| 1 | Cold start → popup visible            | <300 ms      | §1                                           |
| 2 | Warm hotkey → popup visible           | <100 ms      | §2                                           |
| 3 | Search latency (1k items)             | <50 ms       | §3                                           |
| 4 | Search latency (10k items)            | <150 ms      | §3                                           |
| 5 | Paste latency (Enter → app pasted)    | <60 ms       | §4                                           |
| 6 | RAM idle (popup hidden)               | <100 MB RSS  | §5                                           |
| 7 | RAM with 10k clips loaded             | <250 MB RSS  | §5                                           |
| 8 | Bundle size (MSI + NSIS)              | <15 MB       | After `pnpm tauri build`                     |

---

## §1 — Cold start measurement

"Cold" = the Klipo process is not running, the binary is not cached, Defender
has not seen it recently.

```powershell
# Force cold cache: Klipo not running, OS file cache flushed.
Stop-Process -Name klipo -Force -ErrorAction SilentlyContinue

# Measure: tail logs while launching the binary directly.
$logs = "$env:LOCALAPPDATA\app.klipo.desktop\logs"
$tag  = (Get-Date).ToString("yyyyMMdd-HHmmss")

# Klipo writes its startup info via tracing. The line we want looks like:
#   "tauri runtime ready" — from `klipo::startup`
# Take the timestamp from that line, subtract process start time
# (`(Get-Process klipo).StartTime`).
Start-Process "C:\Path\To\klipo.exe"
Start-Sleep -Seconds 5
$proc  = Get-Process klipo
$start = $proc.StartTime
# Now press Ctrl+Alt+V; the popup `Focused` event triggers the paint.
# Subtract from $start, write to results-<yyyy-mm>.md.
```

For continuous measurement, the `klipo::perf` logging target prints a
`startup_ms=NNN` field once the popup is paintable; tail with:

```powershell
$env:KLIPO_LOG = "info,klipo=debug"
pnpm tauri dev    # for dev builds; release builds log to %APPDATA%\app.klipo.desktop\logs\
```

Record 10 cold-launch samples; report p50 and p95.

## §2 — Warm hotkey latency

Klipo is already running (popup hidden). Press the hotkey; measure from
keypress to the popup being visible AND focused.

```powershell
# Approach A: stopwatch (good enough; humans can react in <100 ms).
# Open StopWatch, start it, press Ctrl+Alt+V, hit Stop the moment the popup paints.

# Approach B: tracing-based.
# Klipo logs "captured foreground hwnd" when the hotkey fires (see
# klipo::hotkey target) and "popup hidden" / "focus" related lines on the
# show side. Diff the timestamps.
```

Repeat 10 times, mixing alt-tabs in between so the popup show path
includes a real foreground change. Report p50 and p95.

## §3 — Search latency

Pre-fill the DB with N clips (use a script or the bench crate's seed
helper). Then in the popup type a query and measure the round trip.

```typescript
// Frontend instrumentation hook — paste into devtools console:
//   const t0 = performance.now();
//   const hits = await window.__TAURI__.core.invoke("search_clips", { query: "needle", limit: 50 });
//   console.log("search_ms=", performance.now() - t0, "hits=", hits.length);
```

Measure 10 distinct queries (mix of common + rare terms). Report p50 + p95
for each corpus size (1k, 10k).

## §4 — Paste latency

Trickier — paste involves SetForegroundWindow + SendInput, so the stopwatch
includes the OS's focus-restore latency.

```text
1. Open Notepad. Click into the editor.
2. Press Ctrl+Alt+V to summon Klipo.
3. Press Enter — the popup hides and Notepad receives the paste.
4. Stopwatch from your Enter keypress to Notepad's first character flash.
```

Klipo's logs show `paste_clip called` and the post-SendInput line; the diff
is the backend portion. Total user-perceived latency is the stopwatch.

Report 10 samples, p50 + p95.

## §5 — RAM measurement

```powershell
Stop-Process -Name klipo -Force -ErrorAction SilentlyContinue
Start-Process "C:\Path\To\klipo.exe"
Start-Sleep -Seconds 30   # let WebView2 warm up + watcher idle out
(Get-Process klipo).WorkingSet64 / 1MB
```

For the "10k clips loaded" variant, pre-fill the DB with 10k synthetic clips
(use bench/src/seed.rs or a Python loop hitting `insert_clip` IPC), then
measure RAM after opening the popup once and scrolling the list to bottom
to force virtualization rendering.

---

## Recording the run

Append to `bench/results-<yyyy-mm>.md` (one file per measurement campaign).
Format:

```markdown
## Run — 2026-MM-DD

| Field          | Value                                                          |
| -------------- | -------------------------------------------------------------- |
| Tester         | name (Aydoğan, etc.)                                           |
| Klipo build    | git-sha or v-tag                                               |
| Machine        | "Surface Laptop 7, Snapdragon X1E, 16 GB"                      |
| OS             | "Windows 11 24H2 build 26100.1234"                             |
| Defender       | on                                                             |
| Sample count   | 10 per metric                                                  |

| Metric                          | p50   | p95   | Budget    | Verdict |
| ------------------------------- | ----- | ----- | --------- | ------- |
| Cold start                      | ms    | ms    | <300 ms   |         |
| Warm hotkey                     | ms    | ms    | <100 ms   |         |
| Search 1k                       | ms    | ms    | <50 ms    |         |
| Search 10k                      | ms    | ms    | <150 ms   |         |
| Paste                           | ms    | ms    | <60 ms    |         |
| RAM idle                        | MB    | MB    | <100 MB   |         |
| RAM with 10k clips              | MB    | MB    | <250 MB   |         |
| Bundle: MSI + NSIS combined     | MB    | —     | <15 MB ea |         |

### Verdict

If ≥1 metric misses budget by ≥10%, **do not tag the release** — open an
issue and link it from the next run.
```

---

## Anti-patterns to avoid

1. **Measuring debug builds.** Always `pnpm tauri build` then test the
   produced binary. Debug builds are 5–20× slower and the numbers mean
   nothing.
2. **Ignoring Defender.** First launch on a fresh machine is dominated by
   real-time scan. Repeat after Defender has indexed the binary.
3. **Single-sample reporting.** Always 10 samples minimum; one number is
   noise.
4. **Mixing cold and warm.** Either measure cold consistently or warm
   consistently; document which.
5. **Different machines for different metrics.** Run the whole runbook on
   one machine per session so the numbers are comparable.
