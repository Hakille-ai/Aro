use aro_skills::Skill;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredPluginSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub skill_path: PathBuf,
    pub skill: Skill,
}

/// Discovers skills within a plugin package according to the agent-plugins.org specification:
/// 1. Only immediate child directories under `skills/` are examined.
/// 2. A child directory is only recognized as a skill if `SKILL.md` exists and is a regular file.
/// 3. Non-recursive: no sub-descendants are crawled.
/// 4. Failure isolation: malformed skills are skipped with a warning, not failing the plugin.
pub fn discover_plugin_skills(plugin_root: &Path) -> Vec<DiscoveredPluginSkill> {
    let skills_dir = plugin_root.join("skills");
    if !skills_dir.is_dir() {
        return Vec::new();
    }

    let mut discovered = Vec::new();

    let entries = match std::fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(
                ?err,
                "failed to read skills directory in {}",
                plugin_root.display()
            );
            return Vec::new();
        }
    };

    for entry in entries.flatten() {
        let child_path = entry.path();
        if !child_path.is_dir() {
            continue;
        }

        let skill_md = child_path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        match Skill::load_from_dir(&child_path) {
            Ok(skill) => {
                discovered.push(DiscoveredPluginSkill {
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    icon: skill.icon.clone(),
                    tags: skill.tags.clone(),
                    skill_path: child_path,
                    skill,
                });
            }
            Err(err) => {
                // Failure isolation: skip the invalid skill and continue
                tracing::warn!(
                    ?err,
                    skill_dir = %child_path.display(),
                    "Skipping malformed skill in plugin (failure isolation)"
                );
            }
        }
    }

    discovered
}
