use aro_vector::{reciprocal_rank_fusion, VectorMemoryHit};
use uuid::Uuid;

#[test]
fn test_rrf_empty_hits_all_permutations() {
    let empty_fts: Vec<Uuid> = vec![];
    let empty_vec: Vec<VectorMemoryHit> = vec![];

    // 1. Both empty
    let res = reciprocal_rank_fusion(&empty_fts, &empty_vec, 60.0, 0.40, 0.60);
    assert!(res.is_empty(), "Empty inputs must yield empty output");

    // 2. Empty FTS only
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let vec_hits = vec![
        VectorMemoryHit { memory_id: id1, score: 0.95 },
        VectorMemoryHit { memory_id: id2, score: 0.85 },
    ];
    let res_vec_only = reciprocal_rank_fusion(&empty_fts, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res_vec_only.len(), 2);
    assert_eq!(res_vec_only[0].memory_id, id1);
    assert_eq!(res_vec_only[0].vec_rank, Some(1));
    assert_eq!(res_vec_only[0].fts_rank, None);
    let expected_score_1 = 0.60 / (60.0 + 1.0);
    assert!((res_vec_only[0].rrf_score - expected_score_1).abs() < 1e-6);

    assert_eq!(res_vec_only[1].memory_id, id2);
    assert_eq!(res_vec_only[1].vec_rank, Some(2));
    assert_eq!(res_vec_only[1].fts_rank, None);
    let expected_score_2 = 0.60 / (60.0 + 2.0);
    assert!((res_vec_only[1].rrf_score - expected_score_2).abs() < 1e-6);

    // 3. Empty Vec only
    let fts_hits = vec![id2, id1];
    let res_fts_only = reciprocal_rank_fusion(&fts_hits, &empty_vec, 60.0, 0.40, 0.60);
    assert_eq!(res_fts_only.len(), 2);
    assert_eq!(res_fts_only[0].memory_id, id2);
    assert_eq!(res_fts_only[0].fts_rank, Some(1));
    assert_eq!(res_fts_only[0].vec_rank, None);
    let expected_fts_1 = 0.40 / (60.0 + 1.0);
    assert!((res_fts_only[0].rrf_score - expected_fts_1).abs() < 1e-6);

    assert_eq!(res_fts_only[1].memory_id, id1);
    assert_eq!(res_fts_only[1].fts_rank, Some(2));
    assert_eq!(res_fts_only[1].vec_rank, None);
}

#[test]
fn test_rrf_disjoint_hits_weight_proportions() {
    let f1 = Uuid::new_v4();
    let f2 = Uuid::new_v4();
    let f3 = Uuid::new_v4();

    let v1 = Uuid::new_v4();
    let v2 = Uuid::new_v4();
    let v3 = Uuid::new_v4();

    let fts_hits = vec![f1, f2, f3];
    let vec_hits = vec![
        VectorMemoryHit { memory_id: v1, score: 0.99 },
        VectorMemoryHit { memory_id: v2, score: 0.88 },
        VectorMemoryHit { memory_id: v3, score: 0.77 },
    ];

    // Standard weights: w_vec = 0.60, w_fts = 0.40, k = 60
    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res.len(), 6);
    let ids: Vec<Uuid> = res.iter().map(|s| s.memory_id).collect();
    assert_eq!(ids, vec![v1, v2, v3, f1, f2, f3]);

    // Reversed weights: w_vec = 0.40, w_fts = 0.60
    let res_rev = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.60, 0.40);
    let ids_rev: Vec<Uuid> = res_rev.iter().map(|s| s.memory_id).collect();
    assert_eq!(ids_rev, vec![f1, f2, f3, v1, v2, v3]);
}

#[test]
fn test_rrf_partially_overlapping_consensus_priority() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();
    let e = Uuid::new_v4();
    let f = Uuid::new_v4();

    let fts_hits = vec![a, b, c, d];
    let vec_hits = vec![
        VectorMemoryHit { memory_id: e, score: 0.99 },
        VectorMemoryHit { memory_id: c, score: 0.90 },
        VectorMemoryHit { memory_id: b, score: 0.80 },
        VectorMemoryHit { memory_id: f, score: 0.70 },
    ];

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res.len(), 6);

    assert_eq!(res[0].memory_id, c);
    assert_eq!(res[1].memory_id, b);
    assert_eq!(res[2].memory_id, e);
    assert_eq!(res[3].memory_id, f);
    assert_eq!(res[4].memory_id, a);
    assert_eq!(res[5].memory_id, d);

    let min_consensus_score = res[0].rrf_score.min(res[1].rrf_score);
    let max_unimodal_score = res[2..].iter().map(|s| s.rrf_score).fold(0.0_f32, f32::max);
    assert!(
        min_consensus_score > max_unimodal_score,
        "Consensus hits must rank strictly higher than unimodal hits"
    );
}

#[test]
fn test_rrf_completely_identical_hits_ranking() {
    let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let fts_hits = ids.clone();
    let vec_hits: Vec<VectorMemoryHit> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| VectorMemoryHit {
            memory_id: id,
            score: 1.0 - (i as f32 * 0.1),
        })
        .collect();

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res.len(), 5);

    for (i, item) in res.iter().enumerate() {
        assert_eq!(item.memory_id, ids[i]);
        assert_eq!(item.fts_rank, Some(i + 1));
        assert_eq!(item.vec_rank, Some(i + 1));
        let expected = (0.40 + 0.60) / (60.0 + (i + 1) as f32);
        assert!((item.rrf_score - expected).abs() < 1e-6);
    }
}

#[test]
fn test_rrf_mathematical_monotonicity() {
    let mut fts_hits = Vec::new();
    let mut vec_hits = Vec::new();

    let ids: Vec<Uuid> = (0..30).map(|_| Uuid::new_v4()).collect();
    for (i, &id) in ids.iter().enumerate() {
        fts_hits.push(id);
        vec_hits.push(VectorMemoryHit {
            memory_id: id,
            score: 1.0 / (i + 1) as f32,
        });
    }

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res.len(), 30);

    for i in 0..res.len() - 1 {
        assert!(
            res[i].rrf_score >= res[i + 1].rrf_score,
            "RRF scores must decrease monotonically"
        );
    }

    for (i, item_x) in res.iter().enumerate() {
        for item_y in &res[i + 1..] {
            if let (Some(fx), Some(fy), Some(vx), Some(vy)) =
                (item_x.fts_rank, item_y.fts_rank, item_x.vec_rank, item_y.vec_rank)
            {
                if fx <= fy && vx <= vy && (fx < fy || vx < vy) {
                    assert!(
                        item_x.rrf_score > item_y.rrf_score,
                        "Strict dominance must yield strict score superiority"
                    );
                }
            }
        }
    }
}

#[test]
fn test_rrf_tie_breaking_determinism_and_invariance() {
    let id_low = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let id_high = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    // 1. Exact tie in score: w_fts = 0.50, w_vec = 0.50, k = 60
    let fts_hits = vec![id_low, id_high];
    let vec_hits = vec![
        VectorMemoryHit { memory_id: id_high, score: 0.99 },
        VectorMemoryHit { memory_id: id_low, score: 0.88 },
    ];

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.50, 0.50);
    assert_eq!(res.len(), 2);
    assert!((res[0].rrf_score - res[1].rrf_score).abs() < 1e-7);
    assert_eq!(
        res[0].memory_id, id_high,
        "Tie-breaker 1 vec_rank ASC must place id_high before id_low"
    );

    // 2. Exact tie in score and vec_rank (both None):
    // w_vec = 0.0, w_fts = 0.5. fts_tie has id_high at rank 1 and id_low at rank 2.
    let fts_tie = vec![id_high, id_low];
    let vec_empty: Vec<VectorMemoryHit> = vec![];
    let res_fts_tie = reciprocal_rank_fusion(&fts_tie, &vec_empty, 60.0, 0.50, 0.0);
    assert_eq!(res_fts_tie[0].memory_id, id_high);
    assert_eq!(res_fts_tie[1].memory_id, id_low);

    // 3. Exact tie in score with zero weights:
    // fts_rank ASC breaks the tie deterministically (id_high has fts_rank 1, id_low has fts_rank 2).
    let res_zero_tie = reciprocal_rank_fusion(&fts_tie, &vec_empty, 60.0, 0.0, 0.0);
    assert_eq!(
        res_zero_tie[0].memory_id, id_high,
        "Tie-breaker 2 fts_rank ASC must place id_high before id_low"
    );
    assert_eq!(res_zero_tie[1].memory_id, id_low);

    // 4. Repeated execution invariance across 100 runs
    for _ in 0..100 {
        let repeated = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.50, 0.50);
        assert_eq!(repeated[0].memory_id, id_high);
        assert_eq!(repeated[1].memory_id, id_low);
    }
}

#[test]
fn test_rrf_duplicate_ids_best_rank_retention() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    let fts_hits = vec![a, b, a, c, b];
    let vec_hits = vec![
        VectorMemoryHit { memory_id: b, score: 0.95 },
        VectorMemoryHit { memory_id: a, score: 0.85 },
        VectorMemoryHit { memory_id: b, score: 0.75 },
    ];

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);
    assert_eq!(res.len(), 3, "Output must contain exactly 3 unique items");

    let a_entry = res.iter().find(|s| s.memory_id == a).unwrap();
    assert_eq!(a_entry.fts_rank, Some(1), "A must retain minimum fts_rank 1");
    assert_eq!(a_entry.vec_rank, Some(2), "A must retain vec_rank 2");

    let b_entry = res.iter().find(|s| s.memory_id == b).unwrap();
    assert_eq!(b_entry.fts_rank, Some(2), "B must retain minimum fts_rank 2");
    assert_eq!(b_entry.vec_rank, Some(1), "B must retain minimum vec_rank 1");

    let c_entry = res.iter().find(|s| s.memory_id == c).unwrap();
    assert_eq!(c_entry.fts_rank, Some(4), "C must retain minimum fts_rank 4");
    assert_eq!(c_entry.vec_rank, None);
}

#[test]
fn test_rrf_boundary_clamping_negative_k_and_weights() {
    let id = Uuid::new_v4();
    let fts_hits = vec![id];
    let vec_hits = vec![VectorMemoryHit { memory_id: id, score: 0.9 }];

    let res = reciprocal_rank_fusion(&fts_hits, &vec_hits, -500.0, -10.0, -20.0);
    assert_eq!(res.len(), 1);
    assert!(!res[0].rrf_score.is_nan(), "Score must not be NaN");
    assert!(!res[0].rrf_score.is_infinite(), "Score must not be infinite");
    assert_eq!(res[0].rrf_score, 0.0, "Clamped negative weights must yield 0.0");

    let res_zero = reciprocal_rank_fusion(&fts_hits, &vec_hits, 0.0, 0.0, 0.0);
    assert_eq!(res_zero.len(), 1);
    assert_eq!(res_zero[0].rrf_score, 0.0);
}
