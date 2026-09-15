use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::skill::{Skill, SkillOutput, SkillSummary};

#[derive(Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, Skill>>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, skill: Skill) {
        let mut map = self.skills.write().await;
        map.insert(skill.id.clone(), skill);
    }

    pub async fn unregister(&self, skill_id: &str) -> Option<Skill> {
        let mut map = self.skills.write().await;
        map.remove(skill_id)
    }

    pub async fn get(&self, skill_id: &str) -> Option<Skill> {
        let map = self.skills.read().await;
        map.get(skill_id).cloned()
    }

    pub async fn list(&self) -> Vec<SkillSummary> {
        let map = self.skills.read().await;
        map.values().map(|s| s.summary()).collect()
    }

    pub async fn discover_skills_in_dir(&self, skills_root: &Path) -> usize {
        if !skills_root.is_dir() {
            return 0;
        }

        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(skills_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Non-recursive: only immediate children of skills_root containing SKILL.md
                if path.is_dir() && path.join("SKILL.md").is_file() {
                    match Skill::load_from_dir(&path) {
                        Ok(skill) => {
                            self.register(skill).await;
                            count += 1;
                        }
                        Err(err) => {
                            tracing::warn!(?err, "failed to load skill from {}", path.display());
                        }
                    }
                }
            }
        }
        count
    }

    pub async fn invoke(&self, skill_id: &str, input: &Value) -> Result<SkillOutput> {
        let skill = self
            .get(skill_id)
            .await
            .ok_or_else(|| anyhow!("Skill '{}' not found", skill_id))?;

        skill.execute(input).await
    }
}
