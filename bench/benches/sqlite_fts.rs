//! Benchmarks: SQLite + FTS5 throughput against the v0.1 schema.
//!
//! Measures whether the storage stack can hit perf-budget targets:
//!   - Insert 10k text rows in <2s total
//!   - Search 1k clips: <30ms p95 (LIKE), <10ms p95 (FTS5)
//!   - Search 10k clips with FTS5: <50ms p95
//!
//! Run:
//!   cargo bench -p klipo-bench --bench sqlite_fts
//!
//! Results (criterion) land in `target/criterion/`. Summary should be
//! transcribed into `bench/results-<yyyy-mm>.md`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use klipo_bench::{
    bulk_insert, fresh_db, gen_text_clip, insert_clip, seeded_rng, GeneratedClip,
};
use sqlx::SqlitePool;

async fn populate(pool: &SqlitePool, n: usize) {
    let mut rng = seeded_rng(42);
    let mut clips = Vec::with_capacity(n);
    for _ in 0..n {
        clips.push(gen_text_clip(&mut rng));
    }
    bulk_insert(pool, &clips).await;
}

fn bench_insert_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("insert_throughput");

    for size in [100usize, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_custom(|iters| {
                    rt.block_on(async {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let (_dir, pool) = fresh_db("insert_bench").await;
                            let mut rng = seeded_rng(42);
                            let clips: Vec<GeneratedClip> =
                                (0..size).map(|_| gen_text_clip(&mut rng)).collect();
                            let t0 = std::time::Instant::now();
                            bulk_insert(&pool, &clips).await;
                            total += t0.elapsed();
                            pool.close().await;
                        }
                        total
                    })
                });
            },
        );
    }
    group.finish();
}

fn bench_search_fts(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("search_fts");

    for size in [1_000usize, 10_000].iter() {
        let (_dir, pool) = rt.block_on(async {
            let pair = fresh_db("fts_bench").await;
            populate(&pair.1, *size).await;
            pair
        });

        // Insert one known-target clip we will search for.
        rt.block_on(async {
            let target = GeneratedClip {
                id: klipo_bench::uuid_v7(),
                kind: "text",
                content_hash: klipo_bench::sha256_hex(b"unique-marker-token-xyz"),
                text_content: "the unique marker token xyz appears here for benchmark search".into(),
                size_bytes: 65,
                source_app: "Code.exe",
                created_at: klipo_bench::now_ms(),
            };
            insert_clip(&pool, &target).await;
        });

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.iter_custom(|iters| {
                    rt.block_on(async {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let t0 = std::time::Instant::now();
                            let _rows: Vec<(String, String)> = sqlx::query_as(
                                "SELECT c.id, c.text_content
                                 FROM clips c
                                 JOIN clips_fts f ON c.rowid = f.rowid
                                 WHERE clips_fts MATCH ?
                                 ORDER BY rank
                                 LIMIT 50",
                            )
                            .bind("xyz")
                            .fetch_all(&pool)
                            .await
                            .expect("fts query");
                            total += t0.elapsed();
                        }
                        total
                    })
                });
            },
        );
    }
    group.finish();
}

fn bench_search_like(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("search_like");

    for size in [1_000usize, 10_000].iter() {
        let (_dir, pool) = rt.block_on(async {
            let pair = fresh_db("like_bench").await;
            populate(&pair.1, *size).await;
            pair
        });

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.iter_custom(|iters| {
                    rt.block_on(async {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let t0 = std::time::Instant::now();
                            let _rows: Vec<(String, String)> = sqlx::query_as(
                                "SELECT id, text_content
                                 FROM clips
                                 WHERE text_content LIKE ?
                                 ORDER BY created_at DESC
                                 LIMIT 50",
                            )
                            .bind("%xyz%")
                            .fetch_all(&pool)
                            .await
                            .expect("like query");
                            total += t0.elapsed();
                        }
                        total
                    })
                });
            },
        );
    }
    group.finish();
}

fn bench_pinned_first(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = rt.block_on(async {
        let pair = fresh_db("pinned_bench").await;
        populate(&pair.1, 10_000).await;
        // Pin a few rows.
        sqlx::query("UPDATE clips SET pinned = 1 WHERE rowid IN (1, 2, 3, 4, 5)")
            .execute(&pair.1)
            .await
            .unwrap();
        pair
    });

    c.bench_function("list_pinned_first_50", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let t0 = std::time::Instant::now();
                    let _rows: Vec<(String, i64, i64)> = sqlx::query_as(
                        "SELECT id, pinned, created_at
                         FROM clips
                         WHERE deleted_at IS NULL
                         ORDER BY pinned DESC, created_at DESC
                         LIMIT 50",
                    )
                    .fetch_all(&pool)
                    .await
                    .expect("list query");
                    total += t0.elapsed();
                }
                total
            })
        });
    });
}

criterion_group!(
    benches,
    bench_insert_throughput,
    bench_search_fts,
    bench_search_like,
    bench_pinned_first
);
criterion_main!(benches);
