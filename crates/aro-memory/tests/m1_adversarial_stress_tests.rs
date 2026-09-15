use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aro_core::{AssistantMode, Episode};
use aro_memory::SqliteMemoryStore;
use rusqlite::Connection;
use uuid::Uuid;

#[test]
fn test_fk_cascade_deletes_and_fts_consistency() {
    let path = std::env::temp_dir().join(format!("aro-fk-cascade-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("Cascade Conversation", AssistantMode::Chat, None, None)
        .expect("create conversation");

    let ep1 = Episode::new(
        conv.id,
        1,
        10,
        "Discussion on quantum computing architecture",
        vec!["Adopt Qiskit framework".into()],
        vec!["Qiskit".into(), "IBM".into()],
        250,
    );
    let ep2 = Episode::new(
        conv.id,
        11,
        20,
        "Discussion on memory allocation and safety",
        vec!["Use jemalloc".into()],
        vec!["jemalloc".into(), "Rust".into()],
        300,
    );

    store.store_episode(&ep1).expect("store ep1");
    store.store_episode(&ep2).expect("store ep2");

    // Verify stored
    let initial_episodes = store.list_episodes(conv.id).expect("list");
    assert_eq!(initial_episodes.len(), 2);

    // Direct SQLite check before cascade
    let conn = Connection::open(&path).expect("raw open");
    let ep_count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("ep count before");
    assert_eq!(ep_count_before, 2);

    let fts_count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes_fts WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("fts count before");
    assert_eq!(fts_count_before, 2);

    // Delete the conversation -> foreign key ON DELETE CASCADE should fire
    store.delete_conversation(conv.id).expect("delete conversation");

    // Verify conversation deleted
    let conv_listed = store.list_episodes(conv.id).expect("list after delete");
    assert!(conv_listed.is_empty(), "Episodes should be gone via cascade");

    let ep_count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("ep count after");
    assert_eq!(ep_count_after, 0, "episodes table must have 0 rows for deleted conversation");

    // Check FTS table for ghost rows!
    let fts_count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes_fts WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("fts count after");

    println!("FK Cascade Test: FTS count after delete = {}", fts_count_after);
    assert_eq!(
        fts_count_after, 0,
        "episodes_fts virtual table must have 0 rows after cascade deletion (no ghost rows in FTS!)"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_transaction_rollback_and_fts_atomicity() {
    let path = std::env::temp_dir().join(format!("aro-rollback-atomicity-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("Rollback Atomicity", AssistantMode::Chat, None, None)
        .expect("create conversation");

    // Initial episode
    let ep = Episode::new(
        conv.id,
        1,
        10,
        "Original stable summary",
        vec!["Original decision".into()],
        vec!["EntityA".into()],
        120,
    );
    store.store_episode(&ep).expect("store original ep");

    // Verify search matches original
    let results = store.search_episodes(conv.id, "Original", 5).expect("search");
    assert_eq!(results.len(), 1);

    // Now test a rollback on an UPDATE
    let mut conn = Connection::open(&path).expect("open raw");
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;").expect("pragma");

    {
        let tx = conn.transaction().expect("begin tx");
        // Update episode to corrupted content
        tx.execute(
            r#"
            UPDATE episodes
            SET summary = 'CORRUPTED MUTATION SUMMARY',
                key_decisions = '["CorruptedDecision"]'
            WHERE id = ?1
            "#,
            rusqlite::params![ep.id.to_string()],
        )
        .expect("tx update");

        // Verify that inside the transaction, FTS reflects the update via trigger
        let in_tx_fts_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM episodes_fts WHERE summary MATCH 'CORRUPTED'",
                [],
                |row| row.get(0),
            )
            .expect("in tx fts check");
        assert_eq!(in_tx_fts_count, 1);

        // DELIBERATELY ROLL BACK (drop transaction without committing)
    }

    // After rollback:
    // 1. Episode summary must still be original
    let listed = store.list_episodes(conv.id).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].summary, "Original stable summary");

    // 2. FTS search for "CORRUPTED" must return 0 results
    let corrupted_search = store.search_episodes(conv.id, "CORRUPTED", 5).expect("search");
    assert!(corrupted_search.is_empty(), "Rolled back content must not appear in FTS search");

    // 3. FTS search for original must still work
    let original_search = store.search_episodes(conv.id, "Original", 5).expect("search");
    assert_eq!(original_search.len(), 1);
    assert_eq!(original_search[0].id, ep.id);

    // 4. Raw count in episodes_fts must be exactly 1
    let raw_fts_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes_fts WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("raw fts count");
    assert_eq!(raw_fts_count, 1, "There should be exactly 1 row in episodes_fts");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_adversarial_fts5_queries_and_injections() {
    let path = std::env::temp_dir().join(format!("aro-fts-injection-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("FTS Adversarial", AssistantMode::Chat, None, None)
        .expect("create conversation");

    let ep = Episode::new(
        conv.id,
        1,
        10,
        "Target summary with special tokens: alpha, beta, gamma",
        vec!["Decision on SQLite and PostgreSQL".into()],
        vec!["Rust2024".into(), "TokioAsync".into()],
        200,
    );
    store.store_episode(&ep).expect("store ep");

    let long_query = "a".repeat(10000);
    // Adversarial queries that could crash, cause syntax errors, or inject SQL
    let adversarial_inputs = vec![
        "",
        " ",
        "   \t\r\n   ",
        "\"",
        "\"\"\"",
        "\"\"\"\"\"\"",
        "'''''",
        "'; DROP TABLE episodes; --",
        "'; DELETE FROM episodes_fts; --",
        "UNION SELECT * FROM sqlite_master",
        "OR OR OR",
        "AND AND AND",
        "NOT NOT NOT",
        "NEAR(alpha, beta)",
        "NEAR(alpha, beta, 10)",
        "summary:alpha",
        "entities:Rust2024",
        "column_not_exist:value",
        "*",
        "***",
        "*** --- ??? /// ||| ;;; ()",
        "!@#$%^&*()_+-=[]{}|;':,.<>/?",
        "alpha* OR beta*",
        "a", // single char (might be filtered or handled)
        "1",
        "alpha AND (beta OR (gamma AND NOT delta))",
        "🔥🚀💡",
        "日本語テスト",
        "مرحبا بالعالم",
        "русский текст",
        "%_%",
        "%%%",
        "_ _ _",
        &long_query, // very long query string
    ];

    for input in &adversarial_inputs {
        let result = store.search_episodes(conv.id, input, 10);
        assert!(
            result.is_ok(),
            "Search must never panic or return Err on adversarial input {:?}, got: {:?}",
            input,
            result.err()
        );
    }

    // Verify that normal valid search still works after all adversarial attempts
    let normal = store.search_episodes(conv.id, "SQLite", 10).expect("normal search");
    assert_eq!(normal.len(), 1);
    assert_eq!(normal[0].id, ep.id);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_episode_boundary_and_large_payloads() {
    let path = std::env::temp_dir().join(format!("aro-episode-boundary-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("Boundary Conversation", AssistantMode::Chat, None, None)
        .expect("create conv");

    // 1. Episode with massive summary and entity payload (100k characters)
    let large_summary = "A".repeat(50000);
    let large_decisions = (0..500).map(|i| format!("Decision number {} with detail", i)).collect::<Vec<_>>();
    let large_entities = (0..500).map(|i| format!("Entity_{}", i)).collect::<Vec<_>>();

    let ep_large = Episode::new(
        conv.id,
        1,
        100,
        large_summary.clone(),
        large_decisions.clone(),
        large_entities.clone(),
        100000,
    );
    store.store_episode(&ep_large).expect("store large episode");

    let retrieved = store.get_latest_episode(conv.id).expect("get latest").unwrap();
    assert_eq!(retrieved.token_count, 100000);
    assert_eq!(retrieved.summary.len(), 50000);
    assert_eq!(retrieved.key_decisions.len(), 500);
    assert_eq!(retrieved.entities.len(), 500);

    // 2. Search matches inside large entities and decisions
    let found = store.search_episodes(conv.id, "Entity_499", 5).expect("search entity");
    assert_eq!(found.len(), 1);

    // 3. Inverted turns: turn_start > turn_end
    let ep_inverted = Episode::new(
        conv.id,
        500,
        200,
        "Inverted span episode",
        vec![],
        vec![],
        50,
    );
    // store_episode currently allows inverted turn numbers because SQL schema does not enforce CHECK (turn_start <= turn_end)
    let inv_res = store.store_episode(&ep_inverted);
    assert!(inv_res.is_ok(), "Schema currently allows inverted spans without CHECK constraint");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_episode_upsert_idempotency_and_fts_replacement() {
    let path = std::env::temp_dir().join(format!("aro-upsert-idempotency-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("Upsert Test", AssistantMode::Chat, None, None)
        .expect("create conversation");

    let ep_id = Uuid::new_v4();
    let ep_v1 = Episode {
        id: ep_id,
        conversation_id: conv.id,
        turn_start: 1,
        turn_end: 10,
        summary: "Version 1 summary discussing frontend styling".into(),
        key_decisions: vec!["Use TailwindCSS".into()],
        entities: vec!["Tailwind".into(), "Svelte".into()],
        token_count: 100,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    store.store_episode(&ep_v1).expect("store v1");

    // Search matches v1
    assert_eq!(store.search_episodes(conv.id, "TailwindCSS", 5).unwrap().len(), 1);
    assert_eq!(store.search_episodes(conv.id, "GraphQL", 5).unwrap().len(), 0);

    // Upsert with completely new v2 content
    let ep_v2 = Episode {
        id: ep_id,
        conversation_id: conv.id,
        turn_start: 1,
        turn_end: 10,
        summary: "Version 2 summary discussing backend GraphQL queries".into(),
        key_decisions: vec!["Adopt Apollo GraphQL".into()],
        entities: vec!["GraphQL".into(), "Apollo".into()],
        token_count: 200,
        created_at: ep_v1.created_at,
        updated_at: chrono::Utc::now(),
    };

    store.store_episode(&ep_v2).expect("store v2");

    // Search matches v2
    assert_eq!(store.search_episodes(conv.id, "GraphQL", 5).unwrap().len(), 1);
    // Old v1 term must NO LONGER match!
    assert_eq!(
        store.search_episodes(conv.id, "TailwindCSS", 5).unwrap().len(),
        0,
        "Old content must not match after upsert update"
    );

    // Verify FTS table row count is still exactly 1, not 2
    let conn = Connection::open(&path).expect("open raw");
    let fts_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes_fts WHERE conversation_id = ?1",
            rusqlite::params![conv.id.to_string()],
            |row| row.get(0),
        )
        .expect("fts count");
    assert_eq!(fts_count, 1, "There must be exactly 1 row in episodes_fts after upsert");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_wal_concurrency_heavy_stress() {
    let path = std::env::temp_dir().join(format!("aro-wal-heavy-stress-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv1 = store
        .create_conversation("Stress Conv 1", AssistantMode::Chat, None, None)
        .expect("create conv 1");
    let conv2 = store
        .create_conversation("Stress Conv 2", AssistantMode::Chat, None, None)
        .expect("create conv 2");

    // Populate initial seeds
    for conv in [&conv1, &conv2] {
        let seed = Episode::new(
            conv.id,
            1,
            10,
            "Initial seed episode for WAL stress testing",
            vec!["Seed decision".into()],
            vec!["SeedEntity".into()],
            100,
        );
        store.store_episode(&seed).expect("seed");
    }

    let num_readers = 8;
    let num_writers = 3;
    let writes_per_writer = 50;

    let total_reads = Arc::new(AtomicUsize::new(0));
    let total_writes = Arc::new(AtomicUsize::new(0));
    let read_errors = Arc::new(AtomicUsize::new(0));
    let write_errors = Arc::new(AtomicUsize::new(0));

    let stop_readers = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let start_time = Instant::now();

    // Spawn writers
    let mut writer_handles = Vec::new();
    for writer_idx in 0..num_writers {
        let store_writer = store.clone();
        let target_conv_id = if writer_idx % 2 == 0 { conv1.id } else { conv2.id };
        let total_writes_clone = total_writes.clone();
        let write_errors_clone = write_errors.clone();

        let handle = std::thread::spawn(move || {
            for i in 1..=writes_per_writer {
                let start = (writer_idx * 1000) + (i * 10) + 1;
                let end = start + 9;
                let ep = Episode::new(
                    target_conv_id,
                    start,
                    end,
                    format!("Writer {} batch {} testing concurrent throughput and locks", writer_idx, i),
                    vec![format!("WriterDecision_{}_{}", writer_idx, i)],
                    vec![format!("Entity_{}", i)],
                    150,
                );
                match store_writer.store_episode(&ep) {
                    Ok(_) => {
                        total_writes_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("Writer {} error on iter {}: {:?}", writer_idx, i, e);
                        write_errors_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
                // Small jitter
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        writer_handles.push(handle);
    }

    // Spawn readers
    let mut reader_handles = Vec::new();
    for reader_idx in 0..num_readers {
        let store_reader = store.clone();
        let conv_id = if reader_idx % 2 == 0 { conv1.id } else { conv2.id };
        let total_reads_clone = total_reads.clone();
        let read_errors_clone = read_errors.clone();
        let stop_clone = stop_readers.clone();

        let handle = std::thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                // Reader performs a mix of operations: list, search, get_latest
                let list_res = store_reader.list_episodes(conv_id);
                if list_res.is_err() {
                    read_errors_clone.fetch_add(1, Ordering::Relaxed);
                }

                let search_res = store_reader.search_episodes(conv_id, "concurrent", 10);
                if search_res.is_err() {
                    read_errors_clone.fetch_add(1, Ordering::Relaxed);
                }

                let latest_res = store_reader.get_latest_episode(conv_id);
                if latest_res.is_err() {
                    read_errors_clone.fetch_add(1, Ordering::Relaxed);
                }

                total_reads_clone.fetch_add(3, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        reader_handles.push(handle);
    }

    // Wait for all writers to finish
    for handle in writer_handles {
        handle.join().expect("writer finished");
    }

    // Signal readers to stop and join them
    stop_readers.store(true, Ordering::Relaxed);
    for handle in reader_handles {
        handle.join().expect("reader finished");
    }

    let elapsed = start_time.elapsed();
    let completed_writes = total_writes.load(Ordering::SeqCst);
    let completed_reads = total_reads.load(Ordering::SeqCst);
    let err_w = write_errors.load(Ordering::SeqCst);
    let err_r = read_errors.load(Ordering::SeqCst);

    println!(
        "WAL Heavy Stress (with store.store_episode()): {} writes (errors={}), {} reads (errors={}) in {:?}",
        completed_writes, err_w, completed_reads, err_r, elapsed
    );

    let _ = std::fs::remove_file(&path);

    // Assert that errors occurred, demonstrating the concurrency flaw
    assert_eq!(
        err_w, 0,
        "FAIL: SqliteMemoryStore::store_episode suffered {} write locks ('database is locked') under concurrent writers because it uses rusqlite's default BEGIN DEFERRED instead of BEGIN IMMEDIATE",
        err_w
    );
}

#[test]
fn test_immediate_transaction_behavior_solves_writer_contention() {
    let path = std::env::temp_dir().join(format!("aro-wal-immediate-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("store init");

    let conv = store
        .create_conversation("Immediate Test", AssistantMode::Chat, None, None)
        .expect("create conv");

    let num_writers = 3;
    let writes_per_writer = 50;

    let total_writes = Arc::new(AtomicUsize::new(0));
    let write_errors = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    let start_time = Instant::now();

    for writer_idx in 0..num_writers {
        let db_path = path.clone();
        let conv_id = conv.id;
        let total_writes_clone = total_writes.clone();
        let write_errors_clone = write_errors.clone();

        let handle = std::thread::spawn(move || {
            for i in 1..=writes_per_writer {
                let start = (writer_idx * 1000) + (i * 10) + 1;
                let end = start + 9;
                let ep_id = Uuid::new_v4();

                let write_op = || -> Result<(), rusqlite::Error> {
                    let mut conn = Connection::open(&db_path)?;
                    conn.busy_timeout(Duration::from_millis(5000))?;
                    conn.execute_batch(
                        "PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
                    )?;

                    // Use IMMEDIATE transaction behavior to prevent lock upgrade deadlocks
                    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    tx.execute(
                        r#"
                        INSERT INTO episodes (
                            id, conversation_id, turn_start, turn_end,
                            summary, key_decisions, entities, token_count,
                            created_at, updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        "#,
                        rusqlite::params![
                            ep_id.to_string(),
                            conv_id.to_string(),
                            start as i64,
                            end as i64,
                            format!("Summary {} from writer {}", i, writer_idx),
                            "[]",
                            "[]",
                            100,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339(),
                        ],
                    )?;
                    tx.commit()?;
                    Ok(())
                };

                match write_op() {
                    Ok(_) => {
                        total_writes_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("Immediate tx writer error: {:?}", e);
                        write_errors_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("writer finished");
    }

    let elapsed = start_time.elapsed();
    let completed = total_writes.load(Ordering::SeqCst);
    let errors = write_errors.load(Ordering::SeqCst);

    println!(
        "Immediate Transaction Verification: {} writes (errors={}) in {:?}",
        completed, errors, elapsed
    );

    let _ = std::fs::remove_file(&path);

    assert_eq!(errors, 0, "With TransactionBehavior::Immediate, zero write lock errors occur under concurrent writers");
    assert_eq!(completed, num_writers * writes_per_writer);
}

