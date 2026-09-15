use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub parameters: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub has_scripts: bool,
    pub skill_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub skill_id: String,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Option<Value>,
    pub instructions: String,
    pub skill_dir: PathBuf,
    pub scripts: Vec<PathBuf>,
}

impl Skill {
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let skill_md_path = dir.join("SKILL.md");
        if !skill_md_path.is_file() {
            return Err(anyhow!("SKILL.md not found in {}", dir.display()));
        }

        let content = std::fs::read_to_string(&skill_md_path)
            .with_context(|| format!("failed to read {}", skill_md_path.display()))?;

        let (frontmatter, instructions) = parse_skill_markdown(&content).with_context(|| {
            format!("failed to parse frontmatter in {}", skill_md_path.display())
        })?;

        let dir_name = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = if !frontmatter.name.is_empty() {
            slugify(&frontmatter.name)
        } else {
            dir_name
        };

        // Discover scripts in scripts/
        let mut scripts = Vec::new();
        let scripts_dir = dir.join("scripts");
        if scripts_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        scripts.push(path);
                    }
                }
            }
        }

        Ok(Self {
            id,
            name: frontmatter.name,
            description: frontmatter.description,
            icon: frontmatter.icon,
            tags: frontmatter.tags,
            parameters: frontmatter.parameters,
            instructions,
            skill_dir: dir.to_path_buf(),
            scripts,
        })
    }

    pub fn summary(&self) -> SkillSummary {
        SkillSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            icon: self.icon.clone(),
            tags: self.tags.clone(),
            has_scripts: !self.scripts.is_empty(),
            skill_dir: self.skill_dir.display().to_string(),
        }
    }

    pub async fn execute(&self, input: &Value) -> Result<SkillOutput> {
        // If there are executable scripts, attempt to run the primary one
        if let Some(script_path) = self.scripts.first() {
            let extension = script_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            let mut cmd = match extension {
                "py" => {
                    let mut c = Command::new("python");
                    c.arg(script_path);
                    c
                }
                "js" | "mjs" => {
                    let mut c = Command::new("node");
                    c.arg(script_path);
                    c
                }
                "sh" => {
                    let mut c = Command::new("bash");
                    c.arg(script_path);
                    c
                }
                "ps1" => {
                    let mut c = Command::new("powershell");
                    c.arg("-ExecutionPolicy")
                        .arg("Bypass")
                        .arg("-File")
                        .arg(script_path);
                    c
                }
                _ => Command::new(script_path),
            };

            cmd.current_dir(&self.skill_dir);
            let input_str = serde_json::to_string(input)?;
            cmd.arg(&input_str);

            match cmd.output().await {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();
                    Ok(SkillOutput {
                        success,
                        output: if !stdout.is_empty() {
                            stdout
                        } else {
                            stderr.clone()
                        },
                        error: if !success { Some(stderr) } else { None },
                        skill_id: self.id.clone(),
                    })
                }
                Err(err) => Ok(SkillOutput {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute script: {err}")),
                    skill_id: self.id.clone(),
                }),
            }
        } else {
            // Skill is an instruction/prompt-based skill
            let rendered = format!(
                "## Skill: {}\n{}\n\n### Instructions\n{}\n\n### Input Provided\n{}",
                self.name,
                self.description,
                self.instructions,
                serde_json::to_string_pretty(input).unwrap_or_default()
            );
            Ok(SkillOutput {
                success: true,
                output: rendered,
                error: None,
                skill_id: self.id.clone(),
            })
        }
    }
}

fn parse_skill_markdown(content: &str) -> Result<(SkillFrontmatter, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok((
            SkillFrontmatter {
                name: "Untitled Skill".to_string(),
                description: String::new(),
                tags: Vec::new(),
                icon: None,
                parameters: None,
            },
            content.to_string(),
        ));
    }

    let rest = &trimmed[3..];
    let end_idx = rest
        .find("\n---")
        .or_else(|| rest.find("\r\n---"))
        .ok_or_else(|| anyhow!("unterminated frontmatter in SKILL.md"))?;

    let frontmatter_str = &rest[..end_idx].trim();
    let body_start = if rest[end_idx..].starts_with("\r\n---") {
        end_idx + 5
    } else {
        end_idx + 4
    };
    let instructions = rest[body_start..].trim().to_string();

    let mut name = String::new();
    let mut description = String::new();
    let mut tags = Vec::new();
    let mut icon = None;

    // Simple line-based YAML parser for frontmatter
    for line in frontmatter_str.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_lowercase();
            let val = v.trim().trim_matches('"').trim_matches('\'').trim();
            match key.as_str() {
                "name" => name = val.to_string(),
                "description" => description = val.to_string(),
                "icon" => icon = Some(val.to_string()),
                "tags" if val.starts_with('[') && val.ends_with(']') => {
                    let inner = &val[1..val.len() - 1];
                    tags = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    if name.is_empty() {
        name = "Unnamed Skill".to_string();
    }

    Ok((
        SkillFrontmatter {
            name,
            description,
            tags,
            icon,
            parameters: None,
        },
        instructions,
    ))
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
