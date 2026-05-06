# Klipo Performance Budget — v0.1

**Status:** Live document. Numbers tighten as benchmarks land in `bench/`.
**Audience:** Anyone writing performance-sensitive code.

> Klipo must feel instantaneous on Windows: cold start under 300 ms, warm hotkey under 100 ms. If a change breaks budget, it doesn't ship — bench failures are CI gates.

---

## 1. Headline Targets (Hard Gates)

| Metric | Target | Rationale |
|---|---|---|
| Cold start → popup visible | **<300ms p95** | "Cold" = process not running. Tauri+WebView2 init is the floor. |
| Warm hotkey → popup visible | **<100ms p95** | Process running, window hidden. Should feel instantaneous. |
| Search 1k clips | **<50ms p95** | Below human perception threshold. |
| Search 10k clips | **<150ms p95** | Worst-case library user. |
| Insert clip (text, ≤1KB) | **<20ms p95** | Wall-clock from clipboard event to DB row visible. |
| Insert clip (image, ≤2MB) | **<150ms p95** | Includes hash + thumbnail. |
| Paste action (Enter pressed → app pasted) | **<60ms p95** | Window hide + clipboard write + SendInput. |
| RAM idle (no popup, no panel) | **<100MB RSS** | Includes WebView2 baseline. |
| RAM with 10k clips loaded | **<250MB RSS** | Pessimistic — virtualization keeps this near idle. |
| Bundle size (MSI installer) | **<15MB compressed** | Tauri promise; we keep it. |
| Disk footprint (DB + blobs, 10k clips) | **<500MB** | Includes typical image mix. |

**Anti-pattern:** Targets are not budgets you can borrow against. If search is fast, you cannot make insert slow. Each row is independent.

---

## 2. Why These Numbers

Anchored to user perception research and platform floors:

- **<100ms** = Nielsen's threshold for "instant." Below this, the user feels the system is reacting to them.
- **<300ms cold** = Tauri's reported launch on M2 (~150ms) doubled for Windows + cold disk + Defender scan margin.
- **<50ms search** = SQLite FTS5 is capable of <5ms for 10k rows on modern SSD; budget includes IPC + React render.
- **<60ms paste** = SendInput plus WM_FOCUS round trip; below this and target app perceives "no delay."

A clipboard manager you have to wait for is one you stop reaching for. Speed is a feature, not a bonus.

---

## 3. Measurement Methodology

### 3.1 Cold-Start Timer

Tauri's `tauri::Builder::setup` instruments a timestamp `t_setup`. The frontend, on first `useEffect` after popup mount, records `t_visible`. We log `t_visible - process_creation_time`.

`process_creation_time` is from the OS (Windows: `GetProcessTimes`, macOS: `proc_pidinfo`).

```rust
// src-tauri/src/perf.rs
pub fn record_cold_start() -> u64 {
    let creation = process_creation_ms();
    let visible = now_ms();
    let elapsed = visible - creation;
    metrics::histogram!("klipo.cold_start_ms", elapsed as f64);
    elapsed
}
```

Telemetry is **opt-in only**. For internal benchmarking, a debug build dumps to `~/AppData/Roaming/Klipo/perf.jsonl`.

### 3.2 Hotkey Latency

`tauri-plugin-global-shortcut` callback fires → window show command → IPC roundtrip to frontend → React commits paint. We instrument:

```
T0 = hotkey callback fires
T1 = window.show() returns
T2 = frontend receives "shown" event
T3 = first frame painted (requestAnimationFrame after first commit)

latency = T3 - T0
```

Track p50/p95/p99 separately. Below 100ms p95 is the bar; p99 may spike on AV scanning (acceptable up to 200ms).

### 3.3 Search Latency

Pure SQLite call latency in Rust:

```rust
let t0 = Instant::now();
let rows = sqlx::query_as!(
    Clip,
    "SELECT c.* FROM clips c JOIN clips_fts f ON c.rowid = f.rowid \
     WHERE clips_fts MATCH ? ORDER BY rank LIMIT 50",
    query
).fetch_all(&pool).await?;
let elapsed = t0.elapsed();
```

Frontend-perceived latency adds debounce window (we use 50ms input debounce) + IPC + render. Target user-perceived <100ms; SQL-only target <50ms.

### 3.4 Memory

Sampled every 30s by a debug task using `sysinfo` crate. Stored in perf.jsonl. Three samples:
- `idle_after_5min` (after 5min no activity)
- `peak_during_load` (after loading 10k clips into list scrollback)
- `after_panel_close` (we want this near idle, not near peak)

CI runs a synthetic 10k-clip load in a headless integration test and asserts `peak_during_load < 250MB`.

### 3.5 Bundle Size

`pnpm tauri build` produces an `.msi`. CI asserts size with `du -m`:

```yaml
- run: |
    SIZE=$(stat --printf="%s" target/release/bundle/msi/*.msi)
    if [ "$SIZE" -gt 15728640 ]; then
      echo "Bundle exceeds 15MB"; exit 1
    fi
```

---

## 4. Hot Paths (Where Time Lives)

### 4.1 Clipboard Capture Hot Path (Insert)

```
WM_CLIPBOARDUPDATE message arrives
  ↓ ~0.1ms
WindowProc routes to clipboard module (Rust)
  ↓ ~0.5ms (OpenClipboard + IsClipboardFormatAvailable)
Read formats (CF_UNICODETEXT, CF_HBITMAP, CF_HDROP)
  ↓ ~1-5ms text, ~10-30ms image (depends on size)
Compute SHA-256
  ↓ ~1ms for 1KB text, ~5-15ms for 2MB image
Check sensitive regex set
  ↓ ~0.5ms
Lookup excluded apps (foreground process via OpenProcess + QueryFullProcessImageName)
  ↓ ~1-2ms
Hash dedup query (single-row indexed lookup)
  ↓ ~0.2ms
INSERT INTO clips + FTS5 trigger
  ↓ ~5-10ms (WAL fsync; we batch with 100ms group commit)
Emit Tauri event "clip:new"
  ↓ ~1ms IPC
Frontend updates list
  ↓ ~3-10ms render
TOTAL: ~12-65ms text, ~30-150ms image
```

**Optimizations available:**
- Move SHA-256 + regex off the WindowProc thread → channel-based handoff.
- Group commit (100ms window) for INSERT — already in plan.
- Skip sensitive regex if no patterns enabled (default 8 patterns; user can disable).

### 4.2 Search Hot Path

```
User types char → 50ms debounce → invoke('list_clips', { query, limit: 50 })
  ↓ ~0.5ms IPC serialize
Rust: build FTS5 query (escape + tokenize + AND)
  ↓ ~0.2ms
sqlx::query against pool
  ↓ ~5-30ms (FTS5 MATCH + JOIN)
Serialize Vec<Clip> to JSON for frontend
  ↓ ~1-2ms
Frontend: virtual list updates
  ↓ ~3-10ms
TOTAL user-perceived: ~12-90ms (we're well within 100ms)
```

### 4.3 Hotkey-to-Visible Hot Path

```
Hotkey pressed
  ↓ ~5-15ms (Windows GetMessageHook → tauri callback)
Window.show()
  ↓ ~5-30ms (Windows DwmSetWindowAttribute + ShowWindow)
First frame paint
  ↓ ~16ms (next vsync, depends on monitor refresh)
React: ClipsStore subscribes, fetches first 50 clips
  ↓ ~10-30ms (we should be fetching BEFORE show, not after)
TOTAL: ~50-90ms warm
```

Critical optimization: **prefetch.** When app starts, frontend already fetches first 50 clips into Zustand store. On hotkey, popup just shows pre-rendered list. No DB hit on the show path.

---

## 5. Memory Strategy

| Allocation | Approach |
|---|---|
| First 200 clips' decoded data | LRU cache in Rust; serves popup without DB hit |
| Clips 201–10k | Lazy from DB on scroll |
| Image full-resolution data | Never in RAM; streamed from disk only when preview pane visible |
| Image thumbnails (192x192) | LRU cache, max 50 entries |
| FTS5 index | SQLite manages; ~1.2× text size on disk |
| WebView2 baseline | ~50-80MB; immutable cost |
| React tree + Zustand store | <10MB even with 10k clip rows |

**Why LRU 200?** Empirically, users interact with the last ~50 clips. 200 covers most "look back 30 minutes" sessions without DB hit.

**Why not load all 10k into store?** Serializing 10k rows over IPC is ~50MB JSON; defeats the budget.

---

## 6. Disk I/O Strategy

- **WAL mode + synchronous=NORMAL.** Trade durability for ~10× write throughput. We can lose the last few seconds of clipboard on power-cut — acceptable.
- **Group commit:** 100ms window batches inserts. Bulk paste of 50 items into clipboard sequence becomes 1 transaction.
- **Vacuum incremental:** Run `PRAGMA incremental_vacuum` weekly off-peak.
- **Blob storage:** Files in `%APPDATA%\Klipo\blobs\<sha[:2]>\<sha>.<ext>`; OS file cache is our only cache.

---

## 7. Anti-Patterns Banned

- **JSON.stringify of full clip list in IPC.** Always paginate; max 50 rows per fetch.
- **Regex on every keystroke.** Sensitive detection runs once at insert, never on read.
- **Blocking the WindowProc thread.** All heavy work goes to Tokio.
- **Re-rendering whole list on selection change.** Use virtualization + `key` discipline.
- **Loading thumbnails synchronously during scroll.** IntersectionObserver-driven lazy load only.
- **Calling `OpenClipboard` repeatedly.** Once per WM_CLIPBOARDUPDATE; read all formats; close.

---

## 8. Phase A Benchmarks (Prototypes Living in `bench/`)

**Crate goal:** validate that SQLite + FTS5 + sqlx on Windows can hit our targets, BEFORE we commit to that stack for production. If results show 200ms search at 10k, we revisit (e.g., switch to Tantivy).

### 8.1 SQLite + FTS5 Prototype (`bench/benches/sqlite_fts.rs`)

Scenarios benchmarked with `criterion`:

| Bench | What | Pass Criterion |
|---|---|---|
| `insert_1k_text` | Insert 1000 random text rows in single tx | <200ms total |
| `insert_10k_text` | Insert 10k random text rows | <2s total |
| `search_substring_1k` | LIKE '%term%' over 1k rows | <30ms p95 |
| `search_fts_1k` | FTS5 MATCH over 1k rows | <10ms p95 |
| `search_fts_10k` | FTS5 MATCH over 10k rows | <50ms p95 |
| `search_fts_turkish` | FTS5 MATCH on Turkish corpus, ı/i variants | match expected rows |

Output: `bench/results-2026-05.md` (generated table + raw `bench/criterion/` artifacts gitignored).

### 8.2 Tauri 2 Cold-Start Probe

A minimal Tauri 2 app with empty React bundle, measured on a target Windows 11 laptop. Three readings, median reported, plotted across:
- Defender on/off
- Cold (boot) vs cold (no recent launch)
- WebView2 v117 vs v124

Any >400ms cold-start triggers investigation: which dependency is fat, can it be lazy-loaded.

### 8.3 WebView2 RAM Baseline

Empty Tauri app, no clip logic, just `<div>Hello</div>`. Sample RSS at:
- After window show
- After 5min idle
- After 1 GC cycle (force `chrome://gc` if possible)

Result documents how much of our 100MB idle budget is fixed cost vs ours to spend.

---

## 9. Continuous Performance Discipline

- **CI runs benchmarks on every PR** that touches `src-tauri/src/storage` or `src-tauri/src/clipboard`. Regression > 10% blocks merge.
- **Release notes include perf table.** "v0.2.0: cold start 280ms (was 290ms), search 10k 45ms (was 47ms)."
- **Profiling sessions before each minor release** with `cargo flamegraph` + Chrome DevTools (WebView).
- **User-reported perf bug template** asks for OS, RAM, CPU, clip count, and a 5s recording.

---

## 10. Revisit Triggers

This document is a contract, not a wish. We revisit budgets when:

- A new dependency adds >10MB to bundle.
- A platform target needs different numbers (mobile, low-RAM Linux).
- A feature genuinely cannot fit (e.g., on-device LLM for AI features in v0.3+).

When we revisit, we **make the trade-off explicit** in CHANGELOG and update this doc. We do not silently let budgets slip.
