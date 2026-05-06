//! FTS5-backed search over `clips.text_content`.
//!
//! Query path:
//!   - Empty / whitespace-only query → returns `list_clips` (pinned-first).
//!   - Non-empty query → Turkish-fold + tokenize + AND-of-prefix → `MATCH`,
//!     ordered by `bm25(clips_fts)` rank then `created_at` desc.
//!
//! The user's query is **never** passed verbatim into the FTS string. We
//!
//!   1. fold Turkish characters (ı→i, ş→s, ğ→g, ü→u, ö→o, ç→c, lowercase),
//!   2. tokenize on whitespace,
//!   3. strip FTS metacharacters,
//!   4. rebuild as AND-of-prefix.
//!
//! The DB-side trigger (migration 002) folds stored `text_content` the same
//! way before indexing, so `isik` finds `ışık`.

use serde::Serialize;
use sqlx::Row;

use super::clips::{Clip, ClipKind};
use super::error::StorageResult;
use super::Storage;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub clip: Clip,
    /// FTS5 BM25 rank (lower = better). Present only for non-empty queries.
    pub rank: Option<f64>,
}

/// Fold Turkish-specific letters to their nearest ASCII counterpart, then
/// lowercase the rest. Keeps DB index and search query speaking the same
/// alphabet — see `migrations/002_turkish_fts.sql`.
pub fn turkish_fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ı' | 'İ' => 'i',
            'ş' | 'Ş' => 's',
            'ğ' | 'Ğ' => 'g',
            'ü' | 'Ü' => 'u',
            'ö' | 'Ö' => 'o',
            'ç' | 'Ç' => 'c',
            other => other.to_lowercase().next().unwrap_or(other),
        })
        .collect()
}

/// Build an FTS5 MATCH expression from a free-form user query.
///
/// Examples:
///   "fooBar"          -> "foobar*"
///   "  hello world  " -> "hello* AND world*"
///   "a*b OR c"        -> "ab* AND or* AND c*"  (operators neutralized)
///   "ışık ÖĞRETMEN"   -> "isik* AND ogretmen*"
fn build_fts_query(raw: &str) -> Option<String> {
    let folded = turkish_fold(raw);
    let tokens: Vec<String> = folded
        .split_whitespace()
        .map(|tok| {
            // Strip FTS5 metacharacters from each token to avoid syntax errors
            // and operator hijacking. We keep alphanumerics + Unicode letters.
            let cleaned: String = tok
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            cleaned
        })
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}*", s))
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" AND "))
    }
}

impl Storage {
    /// Search across clips. `limit` is clamped at 200 server-side to keep
    /// IPC payloads modest; the UI rarely needs more than ~50 results.
    pub async fn search_clips(&self, query: &str, limit: i64) -> StorageResult<Vec<SearchHit>> {
        let limit = limit.clamp(1, 200);

        let Some(fts_query) = build_fts_query(query) else {
            // Empty query: degrade to recency listing. No rank.
            let clips = self.list_clips(limit, 0).await?;
            return Ok(clips
                .into_iter()
                .map(|clip| SearchHit { clip, rank: None })
                .collect());
        };

        let rows = sqlx::query(
            "SELECT c.id, c.kind, c.content_hash, c.text_content, c.blob_path,
                    c.size_bytes, c.source_app, c.source_url, c.source_window_title,
                    c.created_at, c.pinned, c.sensitive, c.category,
                    bm25(clips_fts) AS rank
             FROM clips c
             JOIN clips_fts f ON c.rowid = f.rowid
             WHERE clips_fts MATCH ?
               AND c.deleted_at IS NULL
             ORDER BY c.pinned DESC, rank, c.created_at DESC
             LIMIT ?",
        )
        .bind(&fts_query)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        rows.iter()
            .map(|row| {
                let clip = Clip {
                    id: row.try_get("id")?,
                    kind: row.try_get::<&str, _>("kind")?.parse::<ClipKind>()?,
                    content_hash: row.try_get("content_hash")?,
                    text_content: row.try_get("text_content")?,
                    blob_path: row.try_get("blob_path")?,
                    size_bytes: row.try_get("size_bytes")?,
                    source_app: row.try_get("source_app")?,
                    source_url: row.try_get("source_url")?,
                    source_window_title: row.try_get("source_window_title")?,
                    created_at: row.try_get("created_at")?,
                    pinned: row.try_get::<i64, _>("pinned")? != 0,
                    sensitive: row.try_get::<i64, _>("sensitive")? != 0,
                    category: row.try_get("category")?,
                };
                let rank: f64 = row.try_get("rank")?;
                Ok(SearchHit {
                    clip,
                    rank: Some(rank),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::clips::NewClip;

    fn text(body: &str, hash: &str) -> NewClip {
        NewClip {
            kind: ClipKind::Text,
            content_hash: hash.to_string(),
            text_content: Some(body.to_string()),
            blob_path: None,
            size_bytes: body.len() as i64,
            source_app: None,
            source_url: None,
            source_window_title: None,
            sensitive: false,
        }
    }

    #[test]
    fn build_fts_query_handles_metacharacters() {
        assert_eq!(build_fts_query(""), None);
        assert_eq!(build_fts_query("   "), None);
        assert_eq!(build_fts_query("hello").as_deref(), Some("hello*"));
        assert_eq!(
            build_fts_query("hello world").as_deref(),
            Some("hello* AND world*"),
        );
        // Operator hijack attempt — operators become bare lowercase tokens.
        let q = build_fts_query("a OR b NEAR c").unwrap();
        assert!(q.contains("a*"));
        assert!(q.contains("or*"));
        assert!(q.contains("near*"));
        assert!(q.contains("c*"));
    }

    #[test]
    fn turkish_fold_mappings() {
        assert_eq!(turkish_fold("ışık"), "isik");
        assert_eq!(turkish_fold("İSTANBUL"), "istanbul");
        assert_eq!(turkish_fold("öğretmen"), "ogretmen");
        assert_eq!(turkish_fold("Çiçek"), "cicek");
        assert_eq!(turkish_fold("Iğdır"), "igdir");
        assert_eq!(turkish_fold("şeftali"), "seftali");
        assert_eq!(turkish_fold("ÜLKE"), "ulke");
        // Mixed: Turkish letters fold, others lowercase.
        assert_eq!(turkish_fold("Ali ışıklar"), "ali isiklar");
    }

    #[tokio::test]
    async fn search_finds_inserted_text() {
        let s = Storage::open_in_memory().await.unwrap();
        let _ = s
            .insert_clip(text("the quick brown fox", "h1"))
            .await
            .unwrap();
        let _ = s.insert_clip(text("lazy dogs sleep", "h2")).await.unwrap();

        let hits = s.search_clips("brown", 50).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0]
            .clip
            .text_content
            .as_deref()
            .unwrap()
            .contains("brown"));
        assert!(hits[0].rank.is_some());
    }

    #[tokio::test]
    async fn empty_query_returns_recency_list() {
        let s = Storage::open_in_memory().await.unwrap();
        let _ = s.insert_clip(text("alpha", "ha")).await.unwrap();
        let _ = s.insert_clip(text("beta", "hb")).await.unwrap();

        let hits = s.search_clips("", 50).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.rank.is_none()));
    }

    #[tokio::test]
    async fn deleted_clips_are_excluded_from_search() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(text("vanish me", "hv")).await.unwrap() {
            super::super::clips::InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.soft_delete(&id).await.unwrap();
        let hits = s.search_clips("vanish", 50).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn unicode_substring_matches() {
        // `dünya` → `dunya` via the diacritic-removing trigger; both query
        // and index speak the folded alphabet.
        let s = Storage::open_in_memory().await.unwrap();
        let _ = s.insert_clip(text("Merhaba dünya", "ht")).await.unwrap();
        let hits = s.search_clips("dunya", 50).await.unwrap();
        assert_eq!(hits.len(), 1, "diacritic-stripped query should match");
    }

    #[tokio::test]
    async fn turkish_dotless_i_matches() {
        // Bug fix from M3.2 manual test: `isik` must find `ışık`.
        let s = Storage::open_in_memory().await.unwrap();
        let _ = s.insert_clip(text("ışık öğretmen", "h1")).await.unwrap();
        let _ = s.insert_clip(text("Iğdır şehri", "h2")).await.unwrap();
        let _ = s.insert_clip(text("çiçek bahçesi", "h3")).await.unwrap();

        // Each query should find exactly one row by hitting the folded form.
        for (query, expected_substring) in [
            ("isik", "ışık"),
            ("ogretmen", "öğretmen"),
            ("igdir", "Iğdır"),
            ("sehri", "şehri"),
            ("cicek", "çiçek"),
            ("bahcesi", "bahçesi"),
        ] {
            let hits = s.search_clips(query, 50).await.unwrap();
            assert!(
                hits.iter().any(|h| h
                    .clip
                    .text_content
                    .as_deref()
                    .unwrap_or("")
                    .contains(expected_substring)),
                "query `{query}` did not match `{expected_substring}` (hits: {})",
                hits.len(),
            );
        }
    }

    #[tokio::test]
    async fn turkish_uppercase_query_matches() {
        // "ISIK" / "İSTANBUL" should find the same rows as "isik" / "istanbul".
        let s = Storage::open_in_memory().await.unwrap();
        let _ = s.insert_clip(text("ışık var", "ha")).await.unwrap();
        let _ = s.insert_clip(text("İstanbul güzel", "hb")).await.unwrap();

        let hits = s.search_clips("ISIK", 50).await.unwrap();
        assert_eq!(hits.len(), 1);
        let hits = s.search_clips("ISTANBUL", 50).await.unwrap();
        assert_eq!(hits.len(), 1);
    }
}
