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
