//! Context packaging and bounded prompt rendering for ARO Agent.
//!
//! Provides structured rendering of multi-tier cognitive memory:
//! - System Instructions (<= 800 tokens)
//! - Long-Term Semantic Knowledge (<= 1600 tokens)
//! - Chronological Episodic Summaries (<= 2000 tokens)
//! - Working Memory Buffer (<= 2400 tokens, N <= 8 turns)

use aro_core::{
    agent::{AgentRun, ContextPack, ContextSource, ToolRef},
    conversation::{ChatMessage, MessageRole},
    estimate_tokens,
    memory::{EpisodeSummary, LongTermMemory},
    runtime::ModelResponseFormat,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{compact_excerpt, render_environment_snapshot, EnvironmentSnapshot};

/// Formats long-term semantic memories into a structured Markdown section.
pub fn render_semantic_section(memories: &[LongTermMemory]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n## Long-Term Semantic Knowledge (Rules, Facts & Preferences):\n");
    for mem in memories {
        let pin_badge = if mem.pinned { " [PINNED]" } else { "" };
        out.push_str(&format!(
            "- [{}{}]: {}\n",
            mem.category, pin_badge, mem.content
        ));
    }
    out
}

/// Formats episodic summaries into a chronological narrative section.
pub fn render_episodic_section(episodes: &[EpisodeSummary]) -> String {
    if episodes.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n## Chronological Episodic History (Past Milestones):\n");
    for ep in episodes {
        out.push_str(&ep.format_for_prompt());
        out.push('\n');
    }
    out
}

/// Renders the complete composite system prompt incorporating:
/// 1. Base system prompt / instructions
/// 2. Long-term semantic knowledge
/// 3. Chronological episodic summaries
/// 4. Agent runtime context (Goal, Environment, Tools from ContextPack)
/// 5. Response format contract
pub fn render_composite_system_prompt(
    base_system_prompt: &str,
    semantic_memories: &[LongTermMemory],
    episodic_summaries: &[EpisodeSummary],
    agent_context: &str,
    response_format: ModelResponseFormat,
) -> String {
    let semantic_block = render_semantic_section(semantic_memories);
    let episodic_block = render_episodic_section(episodic_summaries);

    let response_contract = match response_format {
        ModelResponseFormat::DirectText => {
            "Response contract:\n- Answer the user directly in natural text.\n- Do not wrap the answer in JSON.\n- Use Markdown only when it improves readability.\n- Cite source IDs only when context sources materially affect the answer."
        }
        ModelResponseFormat::AgentActionJson => {
            "Response contract:\nReturn exactly one JSON object and no markdown. Use {\"type\":\"final\",\"content\":\"...\"} when answering the user, {\"type\":\"tool\",\"toolId\":\"core.search.web\",\"input\":{\"query\":\"...\"},\"reason\":\"...\"} or another listed tool when external, current, file, or page context is required, or {\"type\":\"pause\",\"reason\":\"...\"} when user input is required. For URLs the user provides, prefer core.web.page.read. Cite source IDs in final content when context sources matter."
        }
    };

    format!(
        "{base_system_prompt}{semantic_block}{episodic_block}\n\nAgent runtime context:\n{agent_context}\n\n{response_contract}"
    )
}

/// Cognitive Context Builder supporting multi-tier cognitive memory assembly.
#[derive(Debug, Clone)]
pub struct CognitiveContextBuilder {
    pub max_sources: usize,
    pub max_working_turns: usize,
}

impl Default for CognitiveContextBuilder {
    fn default() -> Self {
        Self {
            max_sources: 32,
            max_working_turns: 8,
        }
    }
}

impl CognitiveContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limits(max_sources: usize, max_working_turns: usize) -> Self {
        Self {
            max_sources,
            max_working_turns: max_working_turns.clamp(1, 8),
        }
    }

    /// Builds a ContextPack integrating episodic, semantic, and working memory sources.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &self,
        run: &AgentRun,
        working_history: &[ChatMessage],
        episodic_summaries: &[EpisodeSummary],
        semantic_memories: &[LongTermMemory],
        dynamic_sources: &[ContextSource],
        skills: &[ToolRef],
        builtin_tools: Vec<ToolRef>,
        environment: EnvironmentSnapshot,
    ) -> ContextPack {
        let mut sources = Vec::new();

        // 1. Core run metadata
        sources.push(ContextSource {
            id: format!("run:{}", run.id),
            kind: "run".to_string(),
            title: "Current goal".to_string(),
            excerpt: run.goal.clone(),
            uri: None,
            score: 1.0,
            created_at: Some(run.created_at),
        });

        // 2. Runtime environment snapshot
        sources.push(ContextSource {
            id: format!("environment:{}", run.id),
            kind: "environment".to_string(),
            title: "Runtime environment".to_string(),
            excerpt: render_environment_snapshot(&environment),
            uri: environment
                .workspace_root
                .as_ref()
                .map(|root| format!("workspace://{}", compact_excerpt(root, 240))),
            score: 0.9,
            created_at: Some(environment.timestamp_utc),
        });

        // 3. Working memory (bounded by max_working_turns)
        let turns = working_history
            .iter()
            .rev()
            .take(self.max_working_turns)
            .collect::<Vec<_>>();
        for message in turns.into_iter().rev() {
            sources.push(ContextSource {
                id: format!("message:{}", message.id),
                kind: "message".to_string(),
                title: match message.role {
                    MessageRole::User => "Recent user message".to_string(),
                    MessageRole::Assistant => "Recent assistant message".to_string(),
                    MessageRole::System => "System message".to_string(),
                },
                excerpt: compact_excerpt(&message.content, 600),
                uri: Some(format!("conversation://{}", message.conversation_id)),
                score: 0.85,
                created_at: Some(message.created_at),
            });
        }

        // 4. Episodic summaries (Tier 2)
        for ep in episodic_summaries {
            sources.push(ContextSource {
                id: format!("episode:{}", ep.id),
                kind: "episode".to_string(),
                title: format!("Episode Turns {}-{}", ep.turn_start, ep.turn_end),
                excerpt: ep.summary.clone(),
                uri: Some(format!("episode://{}", ep.id)),
                score: 0.80,
                created_at: Some(ep.created_at),
            });
        }

        // 5. Semantic memories (Tier 3)
        for mem in semantic_memories {
            sources.push(ContextSource {
                id: format!("memory:{}", mem.id),
                kind: "memory".to_string(),
                title: format!("Memory [{}]", mem.category),
                excerpt: mem.content.clone(),
                uri: Some(format!("memory://{}", mem.id)),
                score: mem.salience,
                created_at: Some(mem.created_at),
            });
        }

        // 6. Dynamic tool sources
        for dyn_source in dynamic_sources {
            if sources.len() >= self.max_sources {
                break;
            }
            sources.push(dyn_source.clone());
        }

        // Ensure we do not exceed max_sources ceiling
        sources.truncate(self.max_sources);

        // Combine builtin tools and enabled skills
        let mut all_tools = builtin_tools;
        all_tools.extend(skills.iter().filter(|tool| tool.enabled).cloned());

        let summary = format!(
            "Goal: {}. Environment: {} on {}. Model: {}. Sources: {}. Tools: {}.",
            compact_excerpt(&run.goal, 240),
            environment.runtime,
            environment.platform,
            environment.model_id.as_deref().unwrap_or("not specified"),
            sources.len(),
            all_tools.len()
        );

        let token_estimate = (estimate_tokens(&summary) as u32)
            + sources
                .iter()
                .map(|source| estimate_tokens(&source.excerpt) as u32)
                .sum::<u32>();

        ContextPack {
            id: Uuid::new_v4(),
            run_id: run.id,
            goal: run.goal.clone(),
            summary,
            sources,
            tools: all_tools,
            token_estimate,
            built_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::{
        agent::ToolRef,
        conversation::{AssistantMode, ChatMessage, MessageRole},
        memory::{EpisodeSummary, LongTermMemory},
        runtime::ModelResponseFormat,
        ToolSource,
    };
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_render_semantic_section_empty_and_populated() {
        assert_eq!(render_semantic_section(&[]), "");

        let mut mem1 = LongTermMemory::new("Always use Rust edition 2021", None);
        mem1.pinned = true;
        mem1.category = "system".to_string();

        let mut mem2 = LongTermMemory::new("User prefers dark theme", None);
        mem2.pinned = false;
        mem2.category = "preference".to_string();

        let rendered = render_semantic_section(&[mem1, mem2]);
        assert!(rendered.contains("## Long-Term Semantic Knowledge"));
        assert!(rendered.contains("[system [PINNED]]: Always use Rust edition 2021"));
        assert!(rendered.contains("[preference]: User prefers dark theme"));
    }

    #[test]
    fn test_render_episodic_section_empty_and_populated() {
        assert_eq!(render_episodic_section(&[]), "");

        let ep = EpisodeSummary {
            id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            turn_start: 1,
            turn_end: 10,
            summary: "Configured WAL mode and completed schema setup.".to_string(),
            key_decisions: vec!["Adopted WAL mode".to_string()],
            entities: vec!["SQLite".to_string()],
            token_count: 85,
            created_at: Utc::now(),
        };

        let rendered = render_episodic_section(&[ep]);
        assert!(rendered.contains("## Chronological Episodic History"));
        assert!(rendered.contains("Turns 1-10"));
        assert!(rendered.contains("Configured WAL mode"));
    }

    #[test]
    fn test_render_composite_system_prompt() {
        let base = "You are ARO assistant.";
        let composite_text = render_composite_system_prompt(
            base,
            &[],
            &[],
            "Runtime context",
            ModelResponseFormat::DirectText,
        );
        assert!(composite_text.starts_with(base));
        assert!(composite_text.contains("Agent runtime context:\nRuntime context"));
        assert!(composite_text.contains("Response contract:"));
        assert!(composite_text.contains("Answer the user directly in natural text."));

        let composite_json = render_composite_system_prompt(
            base,
            &[],
            &[],
            "Runtime context",
            ModelResponseFormat::AgentActionJson,
        );
        assert!(composite_json.contains("Return exactly one JSON object and no markdown."));
    }

    #[test]
    fn test_cognitive_context_builder_bounds() {
        let builder = CognitiveContextBuilder::with_limits(10, 4);
        assert_eq!(builder.max_working_turns, 4);
        assert_eq!(builder.max_sources, 10);

        // max_working_turns clamped to max 8
        let builder_clamped = CognitiveContextBuilder::with_limits(20, 15);
        assert_eq!(builder_clamped.max_working_turns, 8);
        assert_eq!(builder_clamped.max_sources, 20);
    }

    #[test]
    fn test_cognitive_context_builder_empty() {
        let builder = CognitiveContextBuilder::default();
        let run = AgentRun::new(
            "Empty run goal",
            AssistantMode::Chat,
            None,
            None,
            None,
            None,
        );
        let env = EnvironmentSnapshot::default();
        let pack = builder.build(&run, &[], &[], &[], &[], &[], vec![], env);

        assert_eq!(pack.run_id, run.id);
        assert_eq!(pack.goal, "Empty run goal");
        assert!(!pack.summary.is_empty());
        assert!(pack.token_estimate > 0);
        assert_eq!(pack.sources.len(), 2); // run + environment
        assert_eq!(pack.tools.len(), 0);
        assert!(pack.built_at <= Utc::now());
    }

    #[test]
    fn test_cognitive_context_builder_full_pack() {
        let builder = CognitiveContextBuilder::default();
        let conv_id = Uuid::new_v4();
        let run = AgentRun::new(
            "Execute architectural refactor",
            AssistantMode::Code,
            Some(conv_id),
            Some("anthropic".to_string()),
            Some("claude-3-5-sonnet".to_string()),
            None,
        );

        let msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            "Please inspect database migrations",
        );

        let ep = EpisodeSummary {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            turn_start: 1,
            turn_end: 10,
            summary: "Completed schema migrations for episodic memory.".to_string(),
            key_decisions: vec!["Use WAL mode".to_string()],
            entities: vec!["SQLite".to_string()],
            token_count: 50,
            created_at: Utc::now(),
        };

        let mut mem = LongTermMemory::new("Strictly enforce 8192 token ceiling", Some(conv_id));
        mem.category = "rule".to_string();
        mem.pinned = true;
        mem.salience = 1.0;

        let dyn_source = ContextSource {
            id: "tool:fs_read".to_string(),
            kind: "tool".to_string(),
            title: "File read result".to_string(),
            excerpt: "crates/aro-agent/src/lib.rs: 2915 lines".to_string(),
            uri: Some("file:///crates/aro-agent/src/lib.rs".to_string()),
            score: 0.95,
            created_at: Some(Utc::now()),
        };

        let tool = ToolRef {
            id: "core.workspace.read".to_string(),
            name: "Workspace Read".to_string(),
            description: "Read workspace file".to_string(),
            source: ToolSource::BuiltIn,
            enabled: true,
            dangerous: false,
            input_schema: serde_json::json!({}),
            aliases: vec![],
        };

        let disabled_skill = ToolRef {
            id: "core.experimental.tool".to_string(),
            name: "Experimental Tool".to_string(),
            description: "Experimental disabled tool".to_string(),
            source: ToolSource::Skill,
            enabled: false,
            dangerous: true,
            input_schema: serde_json::json!({}),
            aliases: vec![],
        };

        let env = EnvironmentSnapshot::desktop_local(
            Some("anthropic".to_string()),
            Some("claude-3-5-sonnet".to_string()),
        );

        let pack = builder.build(
            &run,
            &[msg],
            &[ep],
            &[mem],
            &[dyn_source],
            &[disabled_skill],
            vec![tool],
            env,
        );

        assert_eq!(pack.run_id, run.id);
        assert_eq!(pack.goal, "Execute architectural refactor");
        assert!(pack.summary.contains("Goal: Execute architectural refactor"));
        // Sources: run (1) + environment (1) + working msg (1) + episode (1) + memory (1) + dyn_source (1) = 6
        assert_eq!(pack.sources.len(), 6);
        // Only enabled tools: builtin tool included, disabled skill excluded -> 1 tool
        assert_eq!(pack.tools.len(), 1);
        assert_eq!(pack.tools[0].id, "core.workspace.read");
        assert!(pack.token_estimate > 10);
        assert!(pack.built_at <= Utc::now());
    }

    #[test]
    fn test_cognitive_context_builder_max_sources_truncation() {
        let builder = CognitiveContextBuilder::with_limits(3, 2);
        let conv_id = Uuid::new_v4();
        let run = AgentRun::new("Truncation test", AssistantMode::Chat, None, None, None, None);
        let env = EnvironmentSnapshot::default();

        let msgs = (0..5)
            .map(|i| ChatMessage::new(conv_id, MessageRole::User, format!("Message {}", i)))
            .collect::<Vec<_>>();

        let pack = builder.build(&run, &msgs, &[], &[], &[], &[], vec![], env);
        // max_sources is 3, so even with run + env + 2 working messages (total 4), it truncates to 3
        assert_eq!(pack.sources.len(), 3);
    }
}
