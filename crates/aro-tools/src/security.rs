use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use aro_core::{AroError, PermissionProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionPreset {
    Standard,
    #[serde(rename = "read-only", alias = "readonly", alias = "read_only")]
    ReadOnly,
    Developer,
    Sandbox,
    Custom,
}

impl PermissionPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::ReadOnly => "read-only",
            Self::Developer => "developer",
            Self::Sandbox => "sandbox",
            Self::Custom => "custom",
        }
    }
}

impl fmt::Display for PermissionPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PermissionPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "read-only" | "readonly" | "read_only" | "lecture seule" => Ok(Self::ReadOnly),
            "developer" | "dev" | "autonome" => Ok(Self::Developer),
            "sandbox" | "isolé" | "isole" => Ok(Self::Sandbox),
            "custom" | "personnalisé" | "personnalise" => Ok(Self::Custom),
            other => Err(format!("unknown permission preset '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    Shell,
    Network,
    Write,
    Read,
    Other,
}

pub fn classify_tool(tool_name: &str) -> ToolCategory {
    let lower = tool_name.trim().to_lowercase();

    // 1. Shell & command execution tools (highest sensitivity check)
    if lower.contains("shell")
        || lower.contains("bash")
        || lower.contains("terminal")
        || lower.contains("cmd")
        || lower.contains("exec")
        || lower.contains("run_command")
        || lower.contains("core.shell.execute")
        || lower.contains("core.code.execute")
    {
        return ToolCategory::Shell;
    }

    // 2. Network & external communication tools
    if lower.contains("web")
        || lower.contains("http")
        || lower.contains("download")
        || lower.contains("curl")
        || lower.contains("browser")
        || lower.contains("external.fetch")
    {
        return ToolCategory::Network;
    }

    // 3. File write / mutation tools
    if lower.contains("write")
        || lower.contains("edit")
        || lower.contains("replace")
        || lower.contains("patch")
        || lower.contains("create")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("save")
        || lower.contains("workspace.write")
        || lower.contains("workspace.delete")
        || lower.contains("workspace.replace_in_files")
        || lower.contains("artifact.create")
        || lower.contains("artifact.update")
    {
        return ToolCategory::Write;
    }

    // 4. File read / inspection tools
    if lower.contains("read")
        || lower.contains("view")
        || lower.contains("search")
        || lower.contains("list")
        || lower.contains("inspect")
        || lower.contains("workspace.read")
        || lower.contains("workspace.list_dir")
        || lower.contains("workspace.git_diff")
        || lower.contains("artifact.list")
    {
        return ToolCategory::Read;
    }

    // 5. Other tools
    ToolCategory::Other
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAuthorizationError {
    pub tool_name: String,
    pub reason: String,
    pub preset: PermissionPreset,
}

impl ToolAuthorizationError {
    pub fn new(
        tool_name: impl Into<String>,
        reason: impl Into<String>,
        preset: PermissionPreset,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            reason: reason.into(),
            preset,
        }
    }
}

impl fmt::Display for ToolAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Tool Authorization Denied [{}] under preset '{}': {}",
            self.tool_name, self.preset, self.reason
        )
    }
}

impl std::error::Error for ToolAuthorizationError {}

impl From<ToolAuthorizationError> for AroError {
    fn from(err: ToolAuthorizationError) -> Self {
        AroError::Security(err.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

impl PermissionDecision {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    pub fn allowed_with_reason(reason: impl Into<String>) -> Self {
        Self {
            allowed: true,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolAuthorizationGuard {
    pub preset: PermissionPreset,
    pub profile: Option<PermissionProfile>,
    pub allowed_tools: Option<HashSet<String>>,
    pub denied_tools: Option<HashSet<String>>,
}

impl Default for ToolAuthorizationGuard {
    fn default() -> Self {
        Self::standard()
    }
}

impl ToolAuthorizationGuard {
    pub fn new(preset: PermissionPreset) -> Self {
        Self {
            preset,
            profile: None,
            allowed_tools: None,
            denied_tools: None,
        }
    }

    pub fn standard() -> Self {
        Self::new(PermissionPreset::Standard)
    }

    pub fn read_only() -> Self {
        Self::new(PermissionPreset::ReadOnly)
    }

    pub fn developer() -> Self {
        Self::new(PermissionPreset::Developer)
    }

    pub fn sandbox() -> Self {
        Self::new(PermissionPreset::Sandbox)
    }

    pub fn custom(profile: PermissionProfile) -> Self {
        Self {
            preset: PermissionPreset::Custom,
            profile: Some(profile),
            allowed_tools: None,
            denied_tools: None,
        }
    }

    pub fn with_allowed_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_tools = Some(
            tools
                .into_iter()
                .map(|s| s.into().trim().to_lowercase())
                .collect(),
        );
        self
    }

    pub fn with_denied_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.denied_tools = Some(
            tools
                .into_iter()
                .map(|s| s.into().trim().to_lowercase())
                .collect(),
        );
        self
    }

    pub fn check_permission(&self, tool_name: &str) -> Result<(), ToolAuthorizationError> {
        let decision = self.evaluate(tool_name)?;
        if decision.allowed {
            Ok(())
        } else {
            Err(ToolAuthorizationError::new(
                tool_name,
                decision
                    .reason
                    .unwrap_or_else(|| "Operation forbidden".to_string()),
                self.preset,
            ))
        }
    }

    pub fn evaluate(&self, tool_name: &str) -> Result<PermissionDecision, ToolAuthorizationError> {
        let trimmed = tool_name.trim();
        if trimmed.is_empty() {
            return Err(ToolAuthorizationError::new(
                if tool_name.is_empty() {
                    "<empty>"
                } else {
                    tool_name
                },
                "Tool name cannot be empty",
                self.preset,
            ));
        }

        let normalized = trimmed.to_lowercase();

        // 1. Explicit denied tools list takes highest priority (overrides all presets)
        if let Some(ref denied) = self.denied_tools {
            if denied.contains(&normalized) {
                return Err(ToolAuthorizationError::new(
                    tool_name,
                    "Tool explicitly blacklisted in denied_tools",
                    self.preset,
                ));
            }
        }

        // 2. Explicit allowed tools list (if set) acts as a strict whitelist
        if let Some(ref allowed) = self.allowed_tools {
            if !allowed.contains(&normalized) {
                return Err(ToolAuthorizationError::new(
                    tool_name,
                    "Tool not found in explicit allowed_tools whitelist",
                    self.preset,
                ));
            }
        }

        let category = classify_tool(&normalized);

        // 3. Preset checks
        match self.preset {
            PermissionPreset::Sandbox => match category {
                ToolCategory::Shell => Err(ToolAuthorizationError::new(
                    tool_name,
                    "Command execution is forbidden in Sandbox mode",
                    self.preset,
                )),
                ToolCategory::Write => Err(ToolAuthorizationError::new(
                    tool_name,
                    "File write and mutation operations are forbidden in Sandbox mode",
                    self.preset,
                )),
                ToolCategory::Network => Err(ToolAuthorizationError::new(
                    tool_name,
                    "External network access is forbidden in Sandbox mode",
                    self.preset,
                )),
                ToolCategory::Read if !normalized.contains("context") => {
                    Err(ToolAuthorizationError::new(
                        tool_name,
                        "Local filesystem access is forbidden in Sandbox mode",
                        self.preset,
                    ))
                }
                _ => Ok(PermissionDecision::allowed()),
            },

            PermissionPreset::ReadOnly => match category {
                ToolCategory::Shell => Err(ToolAuthorizationError::new(
                    tool_name,
                    "Command execution is forbidden in Read-Only mode",
                    self.preset,
                )),
                ToolCategory::Write => Err(ToolAuthorizationError::new(
                    tool_name,
                    "File write and mutation operations are forbidden in Read-Only mode",
                    self.preset,
                )),
                ToolCategory::Network => Err(ToolAuthorizationError::new(
                    tool_name,
                    "External network access is forbidden in Read-Only mode",
                    self.preset,
                )),
                ToolCategory::Read | ToolCategory::Other => Ok(PermissionDecision::allowed()),
            },

            PermissionPreset::Developer => Ok(PermissionDecision::allowed()),

            PermissionPreset::Custom => {
                let profile = self.profile.as_ref().ok_or_else(|| {
                    ToolAuthorizationError::new(
                        tool_name,
                        "Custom preset requires an active AgentPermissionProfile",
                        self.preset,
                    )
                })?;

                match category {
                    ToolCategory::Read if !profile.allow_read => Err(ToolAuthorizationError::new(
                        tool_name,
                        format!("File reads prohibited by profile \"{}\"", profile.name),
                        self.preset,
                    )),
                    ToolCategory::Write if !profile.allow_write => {
                        Err(ToolAuthorizationError::new(
                            tool_name,
                            format!("File writes prohibited by profile \"{}\"", profile.name),
                            self.preset,
                        ))
                    }
                    ToolCategory::Shell if !profile.allow_shell => {
                        Err(ToolAuthorizationError::new(
                            tool_name,
                            format!("Shell execution prohibited by profile \"{}\"", profile.name),
                            self.preset,
                        ))
                    }
                    ToolCategory::Network if !profile.allow_network => {
                        Err(ToolAuthorizationError::new(
                            tool_name,
                            format!("Network access prohibited by profile \"{}\"", profile.name),
                            self.preset,
                        ))
                    }
                    _ => Ok(PermissionDecision::allowed()),
                }
            }

            PermissionPreset::Standard => match category {
                ToolCategory::Shell => Ok(PermissionDecision::allowed_with_reason(
                    "Requires user confirmation before execution",
                )),
                _ => Ok(PermissionDecision::allowed()),
            },
        }
    }
}
