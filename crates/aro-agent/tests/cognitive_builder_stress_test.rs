use aro_agent::{
    render_composite_system_prompt,
    CognitiveContextBuilder, EnvironmentSnapshot,
};
use aro_core::{
    agent::{AgentRun, ContextSource, ToolRef},
    conversation::{AssistantMode, ChatMessage, MessageRole},
    memory::{EpisodeSummary, LongTermMemory},
    runtime::ModelResponseFormat,
    ToolSource,
};
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_adversarial_completely_empty_inputs() {
    let builder = CognitiveContextBuilder::default();
    let run = AgentRun::new("", AssistantMode::Chat, None, None, None, None);
    let env = EnvironmentSnapshot::default();

    let pack = builder.build(&run, &[], &[], &[], &[], &[], vec![], env);

    assert_eq!(pack.run_id, run.id);
    assert_eq!(pack.goal, "");
    // Even with empty inputs, run metadata and environment are present
    assert_eq!(pack.sources.len(), 2);
    assert_eq!(pack.sources[0].kind, "run");
    assert_eq!(pack.sources[1].kind, "environment");
    assert_eq!(pack.tools.len(), 0);
    assert!(pack.token_estimate > 0);
}

#[test]
fn test_adversarial_zero_and_one_max_sources_truncation() {
    let conv_id = Uuid::new_v4();
    let run = AgentRun::new("Truncation boundary", AssistantMode::Chat, None, None, None, None);
    let env = EnvironmentSnapshot::default();
    let msgs = vec![
        ChatMessage::new(conv_id, MessageRole::User, "Hello"),
        ChatMessage::new(conv_id, MessageRole::Assistant, "Hi there"),
    ];

    // max_sources = 0
    let builder_zero = CognitiveContextBuilder::with_limits(0, 4);
    let pack_zero = builder_zero.build(&run, &msgs, &[], &[], &[], &[], vec![], env.clone());
    assert_eq!(pack_zero.sources.len(), 0);

    // max_sources = 1
    let builder_one = CognitiveContextBuilder::with_limits(1, 4);
    let pack_one = builder_one.build(&run, &msgs, &[], &[], &[], &[], vec![], env);
    assert_eq!(pack_one.sources.len(), 1);
    assert_eq!(pack_one.sources[0].kind, "run");
}

#[test]
fn test_adversarial_working_history_chronological_ordering_and_sliding_window() {
    let conv_id = Uuid::new_v4();
    let run = AgentRun::new("History ordering", AssistantMode::Chat, None, None, None, None);
    let env = EnvironmentSnapshot::default();

    // Create 50 messages: Turn 0 to Turn 49
    let msgs: Vec<ChatMessage> = (0..50)
        .map(|i| {
            ChatMessage::new(
                conv_id,
                if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                format!("Turn {:02}", i),
            )
        })
        .collect();

    // With max_working_turns = 5, it should retain Turn 45, 46, 47, 48, 49 in ascending order
    let builder = CognitiveContextBuilder::with_limits(50, 5);
    let pack = builder.build(&run, &msgs, &[], &[], &[], &[], vec![], env);

    // Sources: run (1) + environment (1) + 5 messages = 7 sources
    assert_eq!(pack.sources.len(), 7);
    assert_eq!(pack.sources[2].excerpt, "Turn 45");
    assert_eq!(pack.sources[3].excerpt, "Turn 46");
    assert_eq!(pack.sources[4].excerpt, "Turn 47");
    assert_eq!(pack.sources[5].excerpt, "Turn 48");
    assert_eq!(pack.sources[6].excerpt, "Turn 49");
}

#[test]
fn test_adversarial_enabled_vs_disabled_skills_and_tools() {
    let builder = CognitiveContextBuilder::default();
    let run = AgentRun::new("Tools filtering", AssistantMode::Chat, None, None, None, None);
    let env = EnvironmentSnapshot::default();

    let builtin_active = ToolRef {
        id: "core.search".to_string(),
        name: "Search".to_string(),
        description: "Search tool".to_string(),
        source: ToolSource::BuiltIn,
        enabled: true,
        dangerous: false,
        input_schema: serde_json::json!({}),
        aliases: vec![],
    };

    let skill_enabled = ToolRef {
        id: "skill.python".to_string(),
        name: "Python Runner".to_string(),
        description: "Runs Python code".to_string(),
        source: ToolSource::Skill,
        enabled: true,
        dangerous: false,
        input_schema: serde_json::json!({}),
        aliases: vec![],
    };

    let skill_disabled = ToolRef {
        id: "skill.bash".to_string(),
        name: "Bash Runner".to_string(),
        description: "Runs Bash commands".to_string(),
        source: ToolSource::Skill,
        enabled: false,
        dangerous: true,
        input_schema: serde_json::json!({}),
        aliases: vec![],
    };

    let pack = builder.build(
        &run,
        &[],
        &[],
        &[],
        &[],
        &[skill_enabled.clone(), skill_disabled.clone()],
        vec![builtin_active.clone()],
        env,
    );

    // Should include builtin_active and skill_enabled, but omit skill_disabled
    assert_eq!(pack.tools.len(), 2);
    let tool_ids: Vec<String> = pack.tools.into_iter().map(|t| t.id).collect();
    assert!(tool_ids.contains(&"core.search".to_string()));
    assert!(tool_ids.contains(&"skill.python".to_string()));
    assert!(!tool_ids.contains(&"skill.bash".to_string()));
}

#[test]
fn test_adversarial_massive_multi_tier_overflow_truncation() {
    let builder = CognitiveContextBuilder::with_limits(10, 8);
    let conv_id = Uuid::new_v4();
    let run = AgentRun::new("Massive multi tier", AssistantMode::Chat, None, None, None, None);
    let env = EnvironmentSnapshot::default();

    // 20 messages
    let msgs: Vec<ChatMessage> = (0..20)
        .map(|i| ChatMessage::new(conv_id, MessageRole::User, format!("Msg {}", i)))
        .collect();

    // 20 episodes
    let episodes: Vec<EpisodeSummary> = (0..20)
        .map(|i| EpisodeSummary {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            turn_start: (i * 10) + 1,
            turn_end: (i + 1) * 10,
            summary: format!("Episode summary {}", i),
            key_decisions: vec!["Decision".to_string()],
            entities: vec!["Entity".to_string()],
            token_count: 50,
            created_at: Utc::now(),
        })
        .collect();

    // 20 memories
    let memories: Vec<LongTermMemory> = (0..20)
        .map(|i| {
            let mut m = LongTermMemory::new(format!("Memory fact {}", i), Some(conv_id));
            m.salience = 0.9;
            m
        })
        .collect();

    // 20 dynamic sources
    let dyn_sources: Vec<ContextSource> = (0..20)
        .map(|i| ContextSource {
            id: format!("dyn:{}", i),
            kind: "tool".to_string(),
            title: format!("Dynamic title {}", i),
            excerpt: format!("Dynamic excerpt {}", i),
            uri: None,
            score: 0.8,
            created_at: Some(Utc::now()),
        })
        .collect();

    let pack = builder.build(&run, &msgs, &episodes, &memories, &dyn_sources, &[], vec![], env);

    // Strict ceiling of max_sources = 10
    assert_eq!(pack.sources.len(), 10);
}

#[test]
fn test_adversarial_composite_system_prompt_rendering() {
    // Empty case
    let prompt_empty = render_composite_system_prompt(
        "Base system prompt.",
        &[],
        &[],
        "Context line",
        ModelResponseFormat::DirectText,
    );
    assert!(!prompt_empty.contains("Long-Term Semantic Knowledge"));
    assert!(!prompt_empty.contains("Chronological Episodic History"));
    assert!(prompt_empty.contains("Base system prompt."));
    assert!(prompt_empty.contains("Agent runtime context:\nContext line"));
    assert!(prompt_empty.contains("Response contract:"));
    assert!(prompt_empty.contains("Answer the user directly in natural text."));

    // Populated case
    let mut mem = LongTermMemory::new("Rule: 🚀 Always verify empirically", None);
    mem.category = "critical_rule".to_string();
    mem.pinned = true;

    let ep = EpisodeSummary {
        id: Uuid::new_v4(),
        conversation_id: Uuid::new_v4(),
        turn_start: 1,
        turn_end: 10,
        summary: "Established foundational schema.".to_string(),
        key_decisions: vec![],
        entities: vec![],
        token_count: 30,
        created_at: Utc::now(),
    };

    let prompt_populated = render_composite_system_prompt(
        "Base system prompt.",
        &[mem],
        &[ep],
        "Context line",
        ModelResponseFormat::AgentActionJson,
    );
    assert!(prompt_populated.contains("## Long-Term Semantic Knowledge (Rules, Facts & Preferences):"));
    assert!(prompt_populated.contains("[critical_rule [PINNED]]: Rule: 🚀 Always verify empirically"));
    assert!(prompt_populated.contains("## Chronological Episodic History (Past Milestones):"));
    assert!(prompt_populated.contains("### Episode (Turns 1-10):"));
    assert!(prompt_populated.contains("Established foundational schema."));
    assert!(prompt_populated.contains("Return exactly one JSON object and no markdown."));
}
