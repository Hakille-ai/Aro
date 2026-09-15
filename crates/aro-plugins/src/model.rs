use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginStatus {
    Active,
    Inactive,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginSourceType {
    Local,
    Git,
    Marketplace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMcpServerSummary {
    pub name: String,
    pub transport_type: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub has_scripts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    pub root_path: String,
    pub data_path: String,
    pub enabled: bool,
    #[serde(default)]
    pub is_system: bool,
    pub status: PluginStatus,
    pub status_message: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub source: PluginSourceType,
    pub mcp_servers: Vec<PluginMcpServerSummary>,
    pub skills: Vec<PluginSkillSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<crate::marketplace::PluginAuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginRequest {
    pub source: PluginSourceType,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSkillDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCustomPluginRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub mcp_servers: std::collections::HashMap<String, crate::mcp_config::PluginMcpServerConfig>,
    #[serde(default)]
    pub skills: Vec<CustomSkillDefinition>,
}
