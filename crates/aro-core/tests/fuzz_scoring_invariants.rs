use aro_core::conversation::{ChatMessage, MessageRole};
use aro_core::memory::{
    compute_initial_salience, compute_memory_utility, compute_recency_decay,
    Episode, EpisodeSummary, MemoryCategory, WorkingMemoryBuffer, EBBINGHAUS_HALF_LIFE_HOURS,
};

use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;



// ============================================================================
// 1. Initial Salience Fuzzing & Invariants
// ============================================================================

#[test]
fn test_fuzz_salience_empty_and_whitespace_variations() {
    let inputs = vec![
        "",
        " ",
        "   \t  \r\n  \t ",
        "\u{200B}\u{200C}\u{200D}\u{FEFF}",
        "\n\n\n\n\n\n",
        "\t",
    ];

    for input in inputs {
        for pinned in [false, true] {
            for category in [
                MemoryCategory::Personal,
                MemoryCategory::Technical,
                MemoryCategory::System,
                MemoryCategory::Preference,
            ] {
                let score = compute_initial_salience(input, pinned, category);
                assert!(!score.is_nan(), "Salience score must not be NaN");
                assert!(!score.is_infinite(), "Salience score must not be Infinite");
                assert!(
                    (0.1..=1.0).contains(&score),
                    "Salience score {} out of bounds [0.1, 1.0] for input {:?}",
                    score,
                    input
                );

                if pinned {
                    assert!(score >= 0.85, "Pinned empty input must be at least 0.85, got {}", score);
                }
            }
        }
    }
}

#[test]
fn test_fuzz_salience_unicode_and_emojis() {
    let unicode_inputs: Vec<String> = vec![
        "🚀 🦀 💡 💥 🧠 🎯 ⚡ 🔍".to_string(),
        "مرحبا بالعالم - قاعدة بيانات SQLite".to_string(),
        "שלום עולם - זיכרון קוגניטיבי".to_string(),
        "こんにちは世界 - 認知メモリシステム".to_string(),
        "你好世界 - 知识库与向量检索".to_string(),
        "Привет, мир! Интеграция базы данных".to_string(),
        "Accentuation française: déjà, où, forêt, maïs, cœur, mémoire".to_string(),
        "Mixed: 🚀 `aro_core` 🦀 `WorkingMemoryBuffer` 🧠 always remember!".to_string(),
        "Emoji spam: ".to_string() + &"🔥".repeat(1000),
    ];

    for input in unicode_inputs {
        let score_normal = compute_initial_salience(&input, false, MemoryCategory::Personal);
        assert!(!score_normal.is_nan());
        assert!((0.1..=1.0).contains(&score_normal));

        let score_pinned = compute_initial_salience(&input, true, MemoryCategory::Preference);
        assert!(!score_pinned.is_nan());
        assert!((0.1..=1.0).contains(&score_pinned));
        assert!(score_pinned >= 0.85);
    }
}

#[test]
fn test_fuzz_salience_gigantic_text() {
    let large_chunk = "always remember that `SqliteMemoryStore` has strict rules for WAL mode. ";
    let gigantic = large_chunk.repeat(7000); // ~504,000 characters

    let start = std::time::Instant::now();
    let score_unpinned = compute_initial_salience(&gigantic, false, MemoryCategory::Technical);
    let duration = start.elapsed();

    assert!(!score_unpinned.is_nan());
    // 0.35 (base) + 0.10 (technical) + 0.15 (rule: 'always') + 0.15 (cmd: 'remember') + 0.05 (2 entities out of 10 words) = 0.80
    assert!((score_unpinned - 0.80).abs() < 1e-5, "Expected 0.80 for unpinned technical text, got {}", score_unpinned);

    let score_pinned = compute_initial_salience(&gigantic, true, MemoryCategory::Technical);
    assert_eq!(score_pinned, 1.0, "Pinned gigantic text with boosts must clamp to 1.0");

    assert!(duration.as_millis() < 2000, "Gigantic text salience must complete within 2s, took {:?}", duration);
}


#[test]
fn test_fuzz_salience_extreme_entity_densities() {
    // 0% entity density (all lowercase plain words)
    let zero_density = "the quick brown fox jumps over the lazy dog repeatedly without stop";
    let score_zero = compute_initial_salience(zero_density, false, MemoryCategory::Personal);
    assert!((score_zero - 0.40).abs() < 1e-5, "Expected 0.40, got {}", score_zero);

    // 100% entity density via backticks
    let mut backtick_entities = Vec::new();
    for i in 0..100 {
        backtick_entities.push(format!("`Entity_{}`", i));
    }
    let full_density_backticks = backtick_entities.join(" ");
    let score_backticks = compute_initial_salience(&full_density_backticks, false, MemoryCategory::Personal);
    assert!((score_backticks - 0.55).abs() < 1e-5, "Expected 0.55, got {}", score_backticks);

    // 100% entity density via Capitalized words
    let capitalized_words = "Alpha Beta Gamma Delta Epsilon Zeta Eta Theta Iota Kappa Lambda";
    let score_cap = compute_initial_salience(capitalized_words, false, MemoryCategory::Personal);
    assert!((score_cap - 0.55).abs() < 1e-5, "Expected 0.55, got {}", score_cap);

    // 100% entity density via underscores
    let underscore_words = "foo_bar baz_qux system_init data_point thread_pool cache_l1";
    let score_und = compute_initial_salience(underscore_words, false, MemoryCategory::Personal);
    assert!((score_und - 0.55).abs() < 1e-5, "Expected 0.55, got {}", score_und);
}

#[test]
fn test_fuzz_salience_keyword_matrices() {
    let rule_keywords = [
        "always", "never", "must", "shall", "rule", "constraint",
        "forbidden", "mandatory", "require", "strictement", "toujours",
        "jamais", "interdit", "obligatoire", "règle", "regle",
    ];

    let command_keywords = [
        "remember", "don't forget", "do not forget", "keep in mind",
        "note that", "important", "retiens", "souviens-toi", "n'oublie pas",
        "mémorise", "memorise",
    ];

    let base_score = compute_initial_salience("plain sentence here", false, MemoryCategory::Personal);
    assert!((base_score - 0.40).abs() < 1e-5);

    for rk in rule_keywords {
        let text = format!("This is {} for verification", rk);
        let score = compute_initial_salience(&text, false, MemoryCategory::Personal);
        assert!(
            score > base_score,
            "Rule keyword '{}' should increase salience from base 0.40, got {}",
            rk,
            score
        );
    }

    for ck in command_keywords {
        let text = format!("Please {} this setting", ck);
        let score = compute_initial_salience(&text, false, MemoryCategory::Personal);
        assert!(
            score > base_score,
            "Command keyword '{}' should increase salience from base 0.40, got {}",
            ck,
            score
        );
    }
}
// ============================================================================
// 2. Recency Decay Fuzzing & Mathematical Invariants
// ============================================================================

#[test]
fn test_fuzz_decay_exact_half_life_series() {
    let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let test_intervals = vec![
        (0.0, 1.0),
        (168.0, 0.50),
        (336.0, 0.25),
        (504.0, 0.125),
        (672.0, 0.0625),
        (840.0, 0.03125),
        (1008.0, 0.015625),
        (1176.0, 0.0078125),
    ];

    for (hours, expected_val) in test_intervals {
        let now = t0 + Duration::seconds((hours * 3600.0) as i64);
        let decay = compute_recency_decay(t0, now, false);
        assert!(
            (decay - expected_val as f32).abs() < 1e-4,
            "Decay at {} hours failed: expected {}, got {}",
            hours,
            expected_val,
            decay
        );
    }
}

#[test]
fn test_fuzz_decay_zero_elapsed_and_sub_second() {
    let t0 = Utc::now();

    assert_eq!(compute_recency_decay(t0, t0, false), 1.0);

    let t_past_now = t0 - Duration::seconds(1);
    assert_eq!(compute_recency_decay(t0, t_past_now, false), 1.0);

    assert_eq!(compute_recency_decay(t0, t0, true), 1.0);
}

#[test]
fn test_fuzz_decay_future_dates() {
    let t0 = Utc::now();

    let future_offsets = [
        Duration::seconds(1),
        Duration::minutes(5),
        Duration::hours(24),
        Duration::days(30),
        Duration::days(365),
        Duration::days(3650),
    ];

    for offset in future_offsets {
        let last_used = t0 + offset;
        let decay = compute_recency_decay(last_used, t0, false);
        assert_eq!(decay, 1.0, "Future last_used_at must clamp decay to 1.0");
    }
}

#[test]
fn test_fuzz_decay_extreme_past_dates() {
    let t0 = Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap();

    let past_offsets = [
        Duration::days(365),
        Duration::days(3650),
        Duration::days(18250),
        Duration::days(36500),
    ];

    for offset in past_offsets {
        let last_used = t0 - offset;
        let decay = compute_recency_decay(last_used, t0, false);
        assert!(!decay.is_nan(), "Decay must not be NaN for extreme past dates");
        assert!(!decay.is_infinite(), "Decay must not be Infinite");
        assert!((0.0..=1.0).contains(&decay), "Decay must stay in [0.0, 1.0], got {}", decay);
        assert!(decay < 1e-4, "Decay after 1+ years should be virtually 0.0, got {}", decay);
    }
}

#[test]
fn test_fuzz_decay_leap_years() {
    let feb_28_2024 = Utc.with_ymd_and_hms(2024, 2, 28, 0, 0, 0).unwrap();
    let mar_01_2024 = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();

    let leap_seconds = (mar_01_2024 - feb_28_2024).num_seconds();
    assert_eq!(leap_seconds, 48 * 3600, "Leap year Feb 28 to Mar 1 must be 48 hours");

    let decay_leap = compute_recency_decay(feb_28_2024, mar_01_2024, false);
    let expected_leap = (-std::f64::consts::LN_2 * 48.0 / EBBINGHAUS_HALF_LIFE_HOURS).exp() as f32;
    assert!((decay_leap - expected_leap).abs() < 1e-5, "Leap year decay mismatch");

    let feb_28_2023 = Utc.with_ymd_and_hms(2023, 2, 28, 0, 0, 0).unwrap();
    let mar_01_2023 = Utc.with_ymd_and_hms(2023, 3, 1, 0, 0, 0).unwrap();

    let non_leap_seconds = (mar_01_2023 - feb_28_2023).num_seconds();
    assert_eq!(non_leap_seconds, 24 * 3600, "Non-leap year Feb 28 to Mar 1 must be 24 hours");

    let decay_non_leap = compute_recency_decay(feb_28_2023, mar_01_2023, false);
    let expected_non_leap = (-std::f64::consts::LN_2 * 24.0 / EBBINGHAUS_HALF_LIFE_HOURS).exp() as f32;
    assert!((decay_non_leap - expected_non_leap).abs() < 1e-5, "Non-leap year decay mismatch");
}

#[test]
fn test_fuzz_decay_pinned_immunity_all_scenarios() {
    let now = Utc::now();
    let dates = [
        now,
        now - Duration::seconds(1),
        now - Duration::hours(168),
        now - Duration::days(3650),
        now + Duration::days(50),
    ];

    for d in dates {
        assert_eq!(compute_recency_decay(d, now, true), 1.0, "Pinned memory must ALWAYS return 1.0");
    }
}

// ============================================================================
// 3. Memory Utility Fuzzing & Invariants
// ============================================================================

#[test]
fn test_fuzz_utility_monotonicity_and_strict_invariants() {
    let hybrids = [0.0, 0.1, 0.5, 0.9, 1.0, 5.0];
    let saliences = [0.1, 0.3, 0.5, 0.8, 1.0];
    let recencies = [0.0, 0.1, 0.5, 0.8, 1.0];
    let recalls = [0, 1, 2, 5, 10, 50, 100, 1000];

    for &h in &hybrids {
        for &s in &saliences {
            for &r in &recencies {
                for &c in &recalls {
                    let u = compute_memory_utility(h, s, r, c);
                    assert!(!u.is_nan(), "Utility must not be NaN");
                    assert!(!u.is_infinite(), "Utility must not be Infinite");
                    assert!(u >= 0.0, "Utility must be non-negative, got {}", u);
                }
            }
        }
    }

    for &s in &saliences {
        for &r in &recencies {
            for &c in &recalls {
                for i in 0..hybrids.len() - 1 {
                    let u1 = compute_memory_utility(hybrids[i], s, r, c);
                    let u2 = compute_memory_utility(hybrids[i + 1], s, r, c);
                    assert!(u2 > u1, "Utility must be strictly monotonic in hybrid_score");
                }
            }
        }
    }

    for &h in &hybrids {
        for &s in &saliences {
            for &r in &recencies {
                for i in 0..recalls.len() - 1 {
                    let u1 = compute_memory_utility(h, s, r, recalls[i]);
                    let u2 = compute_memory_utility(h, s, r, recalls[i + 1]);
                    assert!(u2 > u1, "Utility must be strictly monotonic in recall_count");
                }
            }
        }
    }
}

#[test]
fn test_fuzz_utility_boundary_conditions() {
    let u_zero = compute_memory_utility(0.0, 0.0, 0.0, 0);
    assert_eq!(u_zero, 0.0, "All-zero inputs must yield 0.0 utility");

    let u_zero_recall = compute_memory_utility(1.0, 1.0, 1.0, 0);
    assert!((u_zero_recall - 0.85).abs() < 1e-5, "Expected 0.85, got {}", u_zero_recall);

    let u_neg_hybrid = compute_memory_utility(-5.0, 0.5, 0.5, 0);
    let u_zero_hybrid = compute_memory_utility(0.0, 0.5, 0.5, 0);
    assert_eq!(u_neg_hybrid, u_zero_hybrid, "Negative hybrid_score should clamp to 0.0");

    let u_max_recall = compute_memory_utility(1.0, 1.0, 1.0, u32::MAX);
    assert!(!u_max_recall.is_nan());
    assert!(!u_max_recall.is_infinite());
    let expected_recall_term = 0.15 * (1.0_f32 + u32::MAX as f32).ln();
    let expected_total = 0.55 + 0.30 + expected_recall_term;
    assert!((u_max_recall - expected_total).abs() < 1e-4);

    let u_high_hybrid = compute_memory_utility(10000.0, 1.0, 1.0, 0);
    assert!((u_high_hybrid - (5500.0 + 0.30)).abs() < 1e-3);
}
// ============================================================================
// 4. WorkingMemoryBuffer Boundary & Stress Tests
// ============================================================================

#[test]
fn test_fuzz_working_memory_buffer_fifo_eviction() {
    let conv_id = Uuid::new_v4();
    let mut buffer = WorkingMemoryBuffer::with_limits(conv_id, 8, 2400);

    for i in 1..=50 {
        let mut msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {}", i),
        );
        msg.token_estimate = Some(20);
        buffer.push_message(msg);

        assert!(buffer.turn_count() <= 8);
        assert_eq!(buffer.turn_count(), i.min(8));
    }

    assert_eq!(buffer.turn_count(), 8);
    for (idx, turn_num) in (43..=50).enumerate() {
        assert_eq!(buffer.messages[idx].content, format!("Turn {}", turn_num));
    }
}

#[test]
fn test_fuzz_working_memory_buffer_edge_capacities() {
    let conv_id = Uuid::new_v4();

    let mut buffer_single = WorkingMemoryBuffer::with_limits(conv_id, 1, 100);
    for i in 1..=5 {
        let msg = ChatMessage::new(conv_id, MessageRole::User, format!("Msg {}", i));
        buffer_single.push_message(msg);
        assert_eq!(buffer_single.turn_count(), 1);
        assert_eq!(buffer_single.messages[0].content, format!("Msg {}", i));
    }

    let mut buffer_zero = WorkingMemoryBuffer::with_limits(conv_id, 0, 100);
    let msg = ChatMessage::new(conv_id, MessageRole::User, "Msg 1".to_string());
    buffer_zero.push_message(msg);
    assert_eq!(buffer_zero.turn_count(), 0);
}

#[test]
fn test_fuzz_working_memory_token_threshold_boundaries() {
    let conv_id = Uuid::new_v4();
    let mut buffer = WorkingMemoryBuffer::with_limits(conv_id, 10, 2400);

    let mut msg1 = ChatMessage::new(conv_id, MessageRole::User, "Huge message".to_string());
    msg1.token_estimate = Some(2400);
    buffer.push_message(msg1);
    assert_eq!(buffer.total_tokens(), 2400);
    assert!(!buffer.exceeds_token_budget());

    let mut msg2 = ChatMessage::new(conv_id, MessageRole::User, "One token".to_string());
    msg2.token_estimate = Some(1);
    buffer.push_message(msg2);
    assert_eq!(buffer.total_tokens(), 2401);
    assert!(buffer.exceeds_token_budget());

    let msg3 = ChatMessage::new(conv_id, MessageRole::Assistant, "Unestimated".to_string());
    buffer.push_message(msg3);
    assert_eq!(buffer.total_tokens(), 2401);
}

#[test]
fn test_fuzz_working_memory_session_variables_and_scratchpad() {
    let conv_id = Uuid::new_v4();
    let mut buffer = WorkingMemoryBuffer::new(conv_id);

    assert_eq!(buffer.max_turns, 8);
    assert_eq!(buffer.max_tokens, 2400);

    buffer.session_variables.insert("active_file".to_string(), "crates/aro-core/src/memory.rs".to_string());
    buffer.session_variables.insert("user_mode".to_string(), "expert".to_string());
    buffer.scratchpad = Some("Refactoring in progress...".to_string());

    assert_eq!(buffer.session_variables.get("active_file").unwrap(), "crates/aro-core/src/memory.rs");
    assert_eq!(buffer.scratchpad.as_deref(), Some("Refactoring in progress..."));
}

// ============================================================================

// 5. Additional Adversarial Stress Tests & Edge Invariants
// ============================================================================

#[test]
fn test_adversarial_salience_extreme_punctuation_and_casing() {
    let punctuation_soup = "!@#$%^&*()_+=-~`{}[]|:;'<>,.?/";
    let score = compute_initial_salience(punctuation_soup, false, MemoryCategory::Personal);
    assert!(!score.is_nan());
    assert!((0.1..=1.0).contains(&score));

    // All uppercase rule keywords
    let uppercase_rules = "ALWAYS NEVER MUST SHALL FORBIDDEN MANDATORY";
    let score_upper = compute_initial_salience(uppercase_rules, false, MemoryCategory::Personal);
    // Base 0.35 + Personal 0.05 + Rule 0.15 + Density 0.15 = 0.70
    assert!((score_upper - 0.70).abs() < 1e-5, "Expected 0.70 for uppercase rule keywords, got {}", score_upper);

    // French accented rule and command keywords
    let french_text = "Règle stricte: souviens-toi de ne jamais oublier d'appliquer cette contrainte";
    let score_french = compute_initial_salience(french_text, false, MemoryCategory::Preference);
    assert!(score_french >= 0.85);
}

#[test]
fn test_adversarial_decay_ancient_history_and_subseconds() {
    let now = Utc::now();

    // Year 1 AD
    let ancient = Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap();
    let decay_ancient = compute_recency_decay(ancient, now, false);
    assert_eq!(decay_ancient, 0.0, "Decay from year 1 AD must be 0.0");

    // Unix epoch 1970
    let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
    let decay_epoch = compute_recency_decay(epoch, now, false);
    assert_eq!(decay_epoch, 0.0, "Decay from 1970 must be 0.0");

    // Sub-second elapsed: 500ms
    let half_sec_ago = now - Duration::milliseconds(500);
    let decay_subsecond = compute_recency_decay(half_sec_ago, now, false);
    assert_eq!(decay_subsecond, 1.0, "Sub-second elapsed time must round to 1.0 (elapsed_seconds <= 0)");
}

#[test]
fn test_adversarial_utility_extreme_scale() {
    // Huge hybrid score: 1,000,000.0
    let u_mega = compute_memory_utility(1_000_000.0, 1.0, 1.0, 100);
    assert!(!u_mega.is_nan());
    assert!(!u_mega.is_infinite());
    assert!(u_mega > 550_000.0);

    // Extreme recall count + extreme hybrid score
    let u_combined = compute_memory_utility(5000.0, 1.0, 1.0, u32::MAX);
    assert!(!u_combined.is_nan());
    assert!(!u_combined.is_infinite());
}

#[test]
fn test_adversarial_episode_span_and_contains() {
    let conv_id = Uuid::new_v4();

    // Normal episode
    let ep = Episode::new(conv_id, 5, 15, "Summary", vec![], vec![], 100);
    assert_eq!(ep.turn_span(), 11);
    assert!(ep.contains_turn(5));
    assert!(ep.contains_turn(15));
    assert!(ep.contains_turn(10));
    assert!(!ep.contains_turn(4));
    assert!(!ep.contains_turn(16));

    // Inverted turn range (turn_end < turn_start)
    let ep_inverted = Episode::new(conv_id, 20, 10, "Summary", vec![], vec![], 50);
    assert_eq!(ep_inverted.turn_span(), 0, "Inverted episode must return 0 span");
    assert!(!ep_inverted.contains_turn(15), "Inverted episode must contain no turns");
    let summary_inverted = EpisodeSummary::from(&ep_inverted);
    assert_eq!(summary_inverted.turn_start, 20);
    assert_eq!(summary_inverted.turn_end, 10);

    // Single turn episode (turn_start == turn_end)
    let ep_single = Episode::new(conv_id, 7, 7, "Single turn", vec![], vec![], 20);
    assert_eq!(ep_single.turn_span(), 1);
    assert!(ep_single.contains_turn(7));
    assert!(!ep_single.contains_turn(6));
    assert!(!ep_single.contains_turn(8));
}
