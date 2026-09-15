use std::sync::Arc;
use std::thread;
use aro_core::LongTermMemory;
use aro_memory::SqliteMemoryStore;
use uuid::Uuid;

fn setup_test_store() -> (SqliteMemoryStore, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("aro-m3-stress-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("Failed to initialize SqliteMemoryStore");
    (store, path)
}

#[test]
fn test_get_memories_by_ids_empty_and_single() {
    let (store, path) = setup_test_store();

    // 1. Empty slice
    let empty_res = store.get_memories_by_ids(&[]).expect("empty slice query");
    assert!(empty_res.is_empty(), "Empty ID slice must return empty vec");

    // 2. Single ID
    let mut mem = LongTermMemory::new("Knowledge item alpha", None);
    mem.category = "personal".to_string();
    store.upsert_memory(&mem).expect("upsert mem");

    let single_res = store.get_memories_by_ids(&[mem.id]).expect("single ID query");
    assert_eq!(single_res.len(), 1);
    assert_eq!(single_res[0].id, mem.id);
    assert_eq!(single_res[0].content, "Knowledge item alpha");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_get_memories_by_ids_1000_plus_chunking() {
    let (store, path) = setup_test_store();

    // Insert 1,250 memories to test across 3 chunks (500 + 500 + 250)
    let total_count = 1250;
    let mut ids = Vec::with_capacity(total_count);

    for i in 0..total_count {
        let mut mem = LongTermMemory::new(format!("Mass item chunk test #{}", i), None);
        mem.category = "benchmark".to_string();
        store.upsert_memory(&mem).expect("upsert memory");
        ids.push(mem.id);
    }

    assert_eq!(ids.len(), total_count);

    // Query all 1,250 IDs at once
    let retrieved = store.get_memories_by_ids(&ids).expect("chunked query");
    assert_eq!(
        retrieved.len(),
        total_count,
        "All 1,250 memories must be retrieved across 500-chunk boundaries"
    );

    // Verify order and identity
    for (i, mem) in retrieved.iter().enumerate() {
        assert_eq!(mem.id, ids[i], "Mismatch at index {}", i);
        assert_eq!(mem.content, format!("Mass item chunk test #{}", i));
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_get_memories_by_ids_strict_order_preservation() {
    let (store, path) = setup_test_store();

    let mut original_memories = Vec::new();
    for i in 0..100 {
        let mem = LongTermMemory::new(format!("Order test memory {}", i), None);
        store.upsert_memory(&mem).expect("upsert");
        original_memories.push(mem);
    }

    // Generate arbitrary permutation / reverse order
    let mut query_ids: Vec<Uuid> = original_memories.iter().map(|m| m.id).collect();
    query_ids.reverse();

    let retrieved = store.get_memories_by_ids(&query_ids).expect("reversed query");
    assert_eq!(retrieved.len(), 100);

    for (i, mem) in retrieved.iter().enumerate() {
        assert_eq!(
            mem.id, query_ids[i],
            "Strict input order must be preserved: expected {}, got {}",
            query_ids[i], mem.id
        );
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_get_memories_by_ids_duplicates_missing_and_deleted() {
    let (store, path) = setup_test_store();

    let m1 = LongTermMemory::new("Mem 1", None);
    let m2 = LongTermMemory::new("Mem 2", None);
    let m3 = LongTermMemory::new("Mem 3", None);

    store.upsert_memory(&m1).expect("upsert m1");
    store.upsert_memory(&m2).expect("upsert m2");
    store.upsert_memory(&m3).expect("upsert m3");

    // Soft delete m2
    store.delete_memory(m2.id).expect("delete m2");

    let non_existent_1 = Uuid::new_v4();
    let non_existent_2 = Uuid::new_v4();

    // Query: [missing, m1, dup m1, deleted m2, m3, dup m1, missing]
    let query = vec![
        non_existent_1,
        m1.id,
        m1.id,
        m2.id,
        m3.id,
        m1.id,
        non_existent_2,
    ];

    let retrieved = store.get_memories_by_ids(&query).expect("query with dup/missing/deleted");

    // Expected: duplicates deduplicated at first occurrence, missing ignored, deleted ignored
    // Result order: [m1, m3]
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0].id, m1.id);
    assert_eq!(retrieved[1].id, m3.id);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_touch_memories_used_sequential_strict_increments() {
    let (store, path) = setup_test_store();

    let mem = LongTermMemory::new("Frequently recalled fact", None);
    store.upsert_memory(&mem).expect("upsert");

    let initial = store.get_memory(mem.id).expect("get").expect("found");
    assert_eq!(initial.recall_count, 0);
    assert!(initial.last_used_at.is_none());

    // Increment 10 times sequentially
    for step in 1..=10 {
        store.touch_memories_used(&[mem.id]).expect("touch");
        let fetched = store.get_memory(mem.id).expect("get").expect("found");
        assert_eq!(
            fetched.recall_count, step,
            "recall_count must increment strictly by 1 at each call"
        );
        assert!(fetched.last_used_at.is_some());
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_touch_memories_used_batch_and_atomicity() {
    let (store, path) = setup_test_store();

    let m1 = LongTermMemory::new("Batch 1", None);
    let m2 = LongTermMemory::new("Batch 2", None);
    let m3 = LongTermMemory::new("Batch 3", None);

    store.upsert_memory(&m1).expect("upsert 1");
    store.upsert_memory(&m2).expect("upsert 2");
    store.upsert_memory(&m3).expect("upsert 3");

    // Batch touch on m1 and m3
    store.touch_memories_used(&[m1.id, m3.id]).expect("batch touch");

    let r1 = store.get_memory(m1.id).expect("get 1").unwrap();
    let r2 = store.get_memory(m2.id).expect("get 2").unwrap();
    let r3 = store.get_memory(m3.id).expect("get 3").unwrap();

    assert_eq!(r1.recall_count, 1);
    assert_eq!(r2.recall_count, 0, "Untouched memory must remain at 0");
    assert_eq!(r3.recall_count, 1);

    // Empty batch should be a no-op
    store.touch_memories_used(&[]).expect("empty touch");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_touch_memories_used_concurrent_multithreaded_strict_increments() {
    let (store, path) = setup_test_store();

    let mem = LongTermMemory::new("Highly concurrent counter test", None);
    store.upsert_memory(&mem).expect("upsert");

    let shared_store = Arc::new(store);
    let target_id = mem.id;

    let num_threads = 8;
    let touches_per_thread = 20; // 8 * 20 = 160 touches total
    let mut handles = Vec::new();

    for _ in 0..num_threads {
        let store_clone = Arc::clone(&shared_store);
        let handle = thread::spawn(move || {
            for _ in 0..touches_per_thread {
                store_clone
                    .touch_memories_used(&[target_id])
                    .expect("concurrent touch failed");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread join failed");
    }

    let final_mem = shared_store.get_memory(target_id).expect("get").expect("found");
    let expected_count = (num_threads * touches_per_thread) as u32;

    assert_eq!(
        final_mem.recall_count, expected_count,
        "Atomic recall count under {} concurrent threads must equal exactly {}",
        num_threads, expected_count
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_touch_memories_used_on_deleted_and_missing_records() {
    let (store, path) = setup_test_store();

    let mem = LongTermMemory::new("Doomed memory", None);
    store.upsert_memory(&mem).expect("upsert");

    // Touch once -> recall_count = 1
    store.touch_memories_used(&[mem.id]).expect("touch before delete");
    let before_delete = store.get_memory(mem.id).expect("get").unwrap();
    assert_eq!(before_delete.recall_count, 1);

    // Soft delete
    store.delete_memory(mem.id).expect("delete");

    // Touch after delete -> should not increment because WHERE deleted_at IS NULL
    store.touch_memories_used(&[mem.id]).expect("touch after delete");

    // Verify in raw SQLite that recall_count is still 1
    let conn = rusqlite::Connection::open(&path).unwrap();
    let raw_recall: u32 = conn
        .query_row(
            "SELECT recall_count FROM memories WHERE id = ?1",
            rusqlite::params![mem.id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw_recall, 1, "Deleted memory recall_count must not increment");

    // Non-existent ID touch
    let missing_id = Uuid::new_v4();
    store.touch_memories_used(&[missing_id]).expect("touch non-existent id");

    let _ = std::fs::remove_file(&path);
}
