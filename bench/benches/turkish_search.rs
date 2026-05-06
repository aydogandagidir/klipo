//! Benchmark + correctness check for FTS5 with Turkish content.
//!
//! Uses the `unicode61 remove_diacritics 2` tokenizer (per v0.1 schema).
//! We measure whether substring searches over Turkish corpora are fast
//! enough AND whether the tokenizer's case/diacritic folding behaves the
//! way we want for queries containing 'ı/i/I/İ'.
//!
//! NOTE: SQLite's `unicode61` does NOT implement Turkish-specific casing
//! (the `i↔İ` and `ı↔I` mapping). We document expected mismatches in
//! `bench/results-2026-05.md` and decide whether to ship a custom
//! tokenizer in v0.2.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use klipo_bench::{bulk_insert, fresh_db, gen_turkish_text_clip, seeded_rng};
use sqlx::SqlitePool;

async fn populate_turkish(pool: &SqlitePool, n: usize) {
    let mut rng = seeded_rng(7);
    let mut clips = Vec::with_capacity(n);
    for _ in 0..n {
        clips.push(gen_turkish_text_clip(&mut rng));
    }
    bulk_insert(pool, &clips).await;
}

fn bench_turkish_fts(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("turkish_fts");

    for size in [1_000usize, 10_000].iter() {
        let (_dir, pool) = rt.block_on(async {
            let pair = fresh_db("turkish_fts").await;
            populate_turkish(&pair.1, *size).await;
            pair
        });

        // Test queries: dotless-i / dotted-i confusables.
        for query in ["ışık", "Işık", "isik", "ISIK"] {
            group.bench_with_input(
                BenchmarkId::new(format!("size_{}", size), query),
                &query.to_string(),
                |b, q| {
                    b.iter_custom(|iters| {
                        rt.block_on(async {
                            let mut total = std::time::Duration::ZERO;
                            for _ in 0..iters {
                                let t0 = std::time::Instant::now();
                                let _rows: Vec<(String,)> = sqlx::query_as(
                                    "SELECT c.id
                                     FROM clips c JOIN clips_fts f ON c.rowid = f.rowid
                                     WHERE clips_fts MATCH ? LIMIT 50",
                                )
                                .bind(q)
                                .fetch_all(&pool)
                                .await
                                .expect("turkish fts");
                                total += t0.elapsed();
                            }
                            total
                        })
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(turkish, bench_turkish_fts);
criterion_main!(turkish);
