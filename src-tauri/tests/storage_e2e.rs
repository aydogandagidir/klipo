//! End-to-end storage integration test.
//!
//! Unit tests in `storage::clips` and `storage::search` use the in-memory
//! variant of SQLite. This file exercises the same paths over a real file on
//! disk — proving that migrations + WAL pragmas + FTS5 triggers behave
//! correctly when the DB is persistent.

use klipo_lib::storage::clips::{ClipKind, InsertOutcome, NewClip};
use klipo_lib::storage::Storage;
use tempfile::TempDir;

fn make_text(body: &str, hash: &str) -> NewClip {
    NewClip {
        kind: ClipKind::Text,
        content_hash: hash.to_string(),
        text_content: Some(body.to_string()),
        blob_path: None,
        size_bytes: body.len() as i64,
        source_app: Some("integration_test.exe".to_string()),
        source_url: None,
        source_window_title: None,
        sensitive: false,
    }
}

#[tokio::test]
async fn end_to_end_lifecycle_on_disk() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("klipo-e2e.db");

    let storage = Storage::open(&db_path).await.expect("open storage");

    // Empty initially
    assert_eq!(storage.count_live().await.unwrap(), 0);

    // Insert + dedup behavior
    let outcome_a = storage.insert_clip(make_text("alpha", "ha")).await.unwrap();
    let id_a = match outcome_a {
        InsertOutcome::Inserted { id } => id,
        other => panic!("expected Inserted, got {other:?}"),
    };
    let outcome_dup = storage.insert_clip(make_text("alpha", "ha")).await.unwrap();
    assert!(matches!(outcome_dup, InsertOutcome::Bumped { .. }));
    assert_eq!(storage.count_live().await.unwrap(), 1);

    // Different hash → new row
    let outcome_b = storage.insert_clip(make_text("beta", "hb")).await.unwrap();
    let id_b = match outcome_b {
        InsertOutcome::Inserted { id } => id,
        other => panic!("expected Inserted, got {other:?}"),
    };
    assert_eq!(storage.count_live().await.unwrap(), 2);

    // Pin + list ordering
    storage.pin_clip(&id_a, true).await.unwrap();
    let listed = storage.list_clips(50, 0).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, id_a, "pinned must be first");

    // Search hits the FTS index on disk
    let hits = storage.search_clips("beta", 50).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].clip.id, id_b);

    // Soft delete + tombstone
    storage.soft_delete(&id_b).await.unwrap();
    assert_eq!(storage.count_live().await.unwrap(), 1);
    let hits = storage.search_clips("beta", 50).await.unwrap();
    assert!(hits.is_empty(), "deleted rows must not appear in search");

    // Database file actually exists on disk
    assert!(db_path.exists(), "klipo-e2e.db should be created");
}

#[tokio::test]
async fn organize_features_on_disk() {
    // Exercises the organize migrations end-to-end on a real file: the label
    // system (auto-seed + manual add + global rename), the title column +
    // rebuilt FTS triggers (folded + searchable), and favorite top-ordering.
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("klipo-organize.db");
    let storage = Storage::open(&db_path).await.expect("open storage");

    let id = match storage
        .insert_clip(make_text("https://bluedev.dev", "hu"))
        .await
        .unwrap()
    {
        InsertOutcome::Inserted { id } => id,
        other => panic!("expected Inserted, got {other:?}"),
    };

    // Auto label seeds, the user adds a custom one.
    storage.link_auto_label(&id, "url").await.unwrap();
    storage.add_label(&id, "müşteri").await.unwrap();
    let mut names: Vec<String> = storage
        .get_clip(&id)
        .await
        .unwrap()
        .labels
        .into_iter()
        .map(|l| l.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["Bağlantı".to_string(), "müşteri".to_string()]);

    // Global rename of the auto label, preserving its color key.
    storage.rename_label("Bağlantı", "Web").await.unwrap();
    let web = storage
        .get_clip(&id)
        .await
        .unwrap()
        .labels
        .into_iter()
        .find(|l| l.name == "Web")
        .expect("renamed label present");
    assert_eq!(web.auto_key.as_deref(), Some("url"));

    // Title is editable AND searchable via the rebuilt, Turkish-folding FTS
    // triggers — "sirket" must find the title "Şirket sitesi".
    storage
        .set_clip_title(&id, Some("Şirket sitesi"))
        .await
        .unwrap();
    let by_title = storage.search_clips("sirket", 50).await.unwrap();
    assert_eq!(by_title.len(), 1, "title word found via folded FTS on disk");
    assert_eq!(by_title[0].clip.title.as_deref(), Some("Şirket sitesi"));

    // Favorite (pinned) floats the clip to the top of the list.
    let _ = storage
        .insert_clip(make_text("second clip", "h2"))
        .await
        .unwrap();
    storage.pin_clip(&id, true).await.unwrap();
    let listed = storage.list_clips(50, 0).await.unwrap();
    assert_eq!(listed[0].id, id, "favorited clip is first");
    assert!(listed[0].pinned);
}
