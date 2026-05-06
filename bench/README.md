# Klipo Phase A Benchmarks

Validates that SQLite + FTS5 + sqlx hits the targets in `../docs/perf-budget.md`
**before** Phase B build commits the stack.

## Why this exists

Most engineering disasters in clipboard managers come from "we'll just use
SQLite, it's fast enough" without ever measuring. We measure first.

## Running

Requires Rust 1.75+ (stable toolchain via rustup).

```bash
# From repo root:
cd bench
cargo bench --bench sqlite_fts
cargo bench --bench turkish_search
```

Criterion writes HTML reports to `target/criterion/` and a textual summary to stdout.

## Pass criteria (Phase A blocker)

| Bench | Pass | Source |
|---|---|---|
| `insert_throughput / 10000` | <2s total | docs/perf-budget.md §1 |
| `search_fts / 10000` | <50ms p95 | docs/perf-budget.md §1 |
| `search_fts / 1000` | <10ms p95 | docs/perf-budget.md §8.1 |
| `search_like / 1000` | <30ms p95 | docs/perf-budget.md §8.1 |
| `list_pinned_first_50` | <5ms p95 | implicit (popup show path) |
| `turkish_fts` | <100ms p95 + correctness notes | docs/perf-budget.md §8.1 |

After running, transcribe the criterion summary into `results-<yyyy-mm>.md`
(one per measurement campaign). The raw `target/criterion/` artifacts are
gitignored.

## Schema source of truth

The schema in `src/lib.rs::SCHEMA_SQL` mirrors v0.1 migration
`src-tauri/src/storage/migrations/001_initial.sql` (which lands in Phase B M2).
Until the migration file exists, `lib.rs` IS the authoritative DDL — when M2
arrives, both must stay in sync.

## What this is NOT

- Not a load test or fuzz test.
- Not a real-app benchmark — no Tauri runtime, no IPC, no React.
- Not a regression suite (yet) — Phase B M2 wires criterion runs into CI.
- Not measuring the production query path verbatim — it measures the kernel
  of the path. UI debounce + IPC is added in Phase B perf instrumentation.
