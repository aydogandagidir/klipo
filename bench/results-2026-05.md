# Klipo Performance Results — 2026-05

This file collects two kinds of measurements:

1. **Release-time perf (`docs/perf-runbook.md` §1–§5)** — measured against
   the production-built binary for each shipped tag. Cold start, warm
   hotkey, search latency, paste latency, RAM. Manual stopwatch +
   tracing logs.
2. **Phase A storage benches (`bench/`)** — `cargo bench` output for
   the SQLite + FTS5 kernel. Criterion-driven, machine-independent.

Each new release campaign appends a `## Run — vX.Y.Z` section under (1).

---

## Run — v0.1.2 (2026-05-06) · Release-time bundle measurement

| Field          | Value                                                          |
| -------------- | -------------------------------------------------------------- |
| Tester         | Aydoğan (CI artifact)                                          |
| Klipo build    | `v0.1.2` (commit `3825728`)                                    |
| Machine        | GitHub Actions `windows-latest` runner                         |
| OS             | Windows Server 2022 / 2025 (CI image)                          |
| Build profile  | release (LTO=true, opt-level=3, codegen-units=1, strip=true)   |

| Metric                            | Value     | Budget     | Verdict |
| --------------------------------- | --------- | ---------- | ------- |
| NSIS installer (`.exe`)           | 3.78 MB   | <15 MB     | ✅ 25%  |
| MSI installer (`.msi`)            | 5.22 MB   | <15 MB     | ✅ 35%  |
| Updater manifest (`latest.json`)  | 779 B     | n/a        | —      |
| Ed25519 sig (per installer)       | 416 B     | n/a        | —      |

> The NSIS bundle ships at ~25% of the perf-budget cap for installer size.
> Headroom is comfortable for adding tessdata for Phase C OCR (~30 MB) or
> WASM modules for snippet preview (~5 MB).

### Deferred — local-machine metrics (require user-side measurement)

These need a real Windows machine running the production MSI/NSIS install
and `KLIPO_LOG=info,klipo::perf=debug` to harvest logs. Maintainer fills in
when the runbook is exercised. See [`docs/perf-runbook.md`](../docs/perf-runbook.md):

| Metric                          | p50 | p95 | Budget    | Verdict |
| ------------------------------- | --- | --- | --------- | ------- |
| Cold start → popup visible      | TBD | TBD | <300 ms   | TBD     |
| Warm hotkey → popup re-focused  | TBD | TBD | <100 ms   | TBD     |
| Search 1k clips                 | TBD | TBD | <50 ms    | TBD     |
| Search 10k clips                | TBD | TBD | <150 ms   | TBD     |
| Paste (Enter → app receives)    | TBD | TBD | <60 ms    | TBD     |
| RAM idle (popup hidden)         | TBD | TBD | <100 MB   | TBD     |
| RAM with 10k clips loaded       | TBD | TBD | <250 MB   | TBD     |

Tracing targets emit the corresponding metrics:

- `klipo::perf` → `popup_visible_ms` (process start → first focus)
- `klipo::perf` → `hotkey_to_focus_ms` (hotkey press → re-focus)
- DevTools console (popup) → `klipo:search_ms` per FTS5 query

Open Klipo with `KLIPO_LOG=info,klipo::perf=debug` set, exercise each path
10 times, drop p50 + p95 into the table.

---

# Phase A Benchmark Results — 2026-05

**Status:** Placeholder. To be filled in once the bench crate compiles and runs on a target Windows machine.

This file is the human-readable summary of `cargo bench` output for the Phase A architecture validation. Format mirrors the table in `README.md` so engineers can compare planned-vs-measured at a glance.

## Run Environment

| Field | Value (placeholder) |
|---|---|
| Date | TBD |
| Machine | TBD (e.g., "Windows 11 Pro 24H2, Ryzen 7 7840HS, 32GB DDR5, Samsung 990 Pro NVMe") |
| Rust toolchain | TBD (e.g., 1.78.0 stable) |
| sqlx version | 0.8.x |
| SQLite version | TBD (`SELECT sqlite_version();`) |
| Defender state | TBD (on/off) |
| Build profile | bench (LTO thin, opt-level 3) |

## Headline Numbers

| Bench | Target | Measured (p95) | Verdict |
|---|---|---|---|
| `insert_throughput / 100` | — | TBD | TBD |
| `insert_throughput / 1000` | — | TBD | TBD |
| `insert_throughput / 10000` | <2.0s total | TBD | TBD |
| `search_fts / 1000` | <10ms | TBD | TBD |
| `search_fts / 10000` | <50ms | TBD | TBD |
| `search_like / 1000` | <30ms | TBD | TBD |
| `search_like / 10000` | <50ms | TBD | TBD |
| `list_pinned_first_50 (10k corpus)` | <5ms | TBD | TBD |
| `turkish_fts size_10000 / "ışık"` | <100ms | TBD | TBD |
| `turkish_fts size_10000 / "Işık"` | <100ms | TBD | TBD |
| `turkish_fts size_10000 / "isik"` | <100ms | TBD | TBD |
| `turkish_fts size_10000 / "ISIK"` | <100ms | TBD | TBD |

## Turkish Tokenizer Correctness Notes

Once measured, fill in the cross-product matrix below. We expect:

- `ışık` matches docs containing `ışık` (exact).
- `Işık` matches `ışık` only after Unicode case-folding handles the dotless/dotted asymmetry — likely **YES** because `unicode61` lowercases via simple Unicode tables (which map `I→i` and `İ→i̇`). This means **Turkish "Işık" (capital dotless I) gets folded to dotted-i incorrectly** for Turkish locale expectations.
- `isik` matches `ışık` after `remove_diacritics 2` strips the dot under `ı`. Expected **YES** but verify.

| Query | Expected match | Observed | Notes |
|---|---|---|---|
| ışık → ışık | yes | TBD | exact |
| Işık → ışık | yes (Unicode-folded) | TBD | Turkish locale would say no; ok for our use |
| isik → ışık | yes (diacritic-stripped) | TBD | depends on remove_diacritics behavior |
| ISIK → ışık | yes | TBD |  |
| ığdır → İstanbul | no | TBD | sanity, no shared tokens |

If the observed behavior diverges from "expected," document the divergence and decide:

1. Accept Unicode-default behavior + caveat in user docs.
2. Implement a custom FTS5 tokenizer in v0.2 with proper Turkish casing.

## Conclusions

(To be written after the run.)

If all targets met → **Phase A storage validation passes, proceed to Phase B M2.**

If any target missed → revisit options:

- Tune SQLite PRAGMAs (e.g., larger cache_size, mmap_size).
- Move FTS5 inserts to a deferred background queue.
- Consider Tantivy as a search index alternative.
- Re-evaluate budget if the miss is small (~10%) — perf may still feel right with UX optimizations.
