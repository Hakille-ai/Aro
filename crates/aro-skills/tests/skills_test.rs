use aro_skills::{Skill, SkillRegistry};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_skill_parsing_and_execution() {
    let temp = tempdir().unwrap();
    let skill_dir = temp.path().join("summarizer");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: Summarizer Skill
description: Summarizes text content into concise bullet points.
tags: [nlp, summary, productivity]
icon: 📝
---
# Summarizer Instructions
Please summarize the input into bullet points:
- Key insight 1
- Key insight 2
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let skill = Skill::load_from_dir(&skill_dir).expect("load skill");
    assert_eq!(skill.id, "summarizer-skill");
    assert_eq!(skill.name, "Summarizer Skill");
    assert_eq!(
        skill.description,
        "Summarizes text content into concise bullet points."
    );
    assert_eq!(skill.tags, vec!["nlp", "summary", "productivity"]);
    assert_eq!(skill.icon.as_deref(), Some("📝"));

    // Invoke instruction skill
    let input = json!({ "text": "This is a long article about AI." });
    let output = skill.execute(&input).await.expect("execute skill");
    assert!(output.success);
    assert!(output.output.contains("Summarizer Skill"));
    assert!(output.output.contains("This is a long article about AI."));
}

#[tokio::test]
async fn test_skill_registry() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("all_skills");
    let s1 = root.join("s1");
    let s2 = root.join("s2");
    fs::create_dir_all(&s1).unwrap();
    fs::create_dir_all(&s2).unwrap();

    fs::write(
        s1.join("SKILL.md"),
        "---\nname: S1\ndescription: First\n---\n# Instr\n",
    )
    .unwrap();
    fs::write(
        s2.join("SKILL.md"),
        "---\nname: S2\ndescription: Second\n---\n# Instr\n",
    )
    .unwrap();

    let registry = SkillRegistry::new();
    let count = registry.discover_skills_in_dir(&root).await;
    assert_eq!(count, 2);

    let list = registry.list().await;
    assert_eq!(list.len(), 2);

    let s1_res = registry.invoke("s1", &json!({})).await.unwrap();
    assert!(s1_res.success);
}
