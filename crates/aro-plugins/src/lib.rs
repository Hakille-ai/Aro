pub mod accounts;
pub mod manager;
pub mod manifest;
pub mod marketplace;
pub mod mcp_config;
pub mod model;
pub mod scaffold;
pub mod skills_discovery;

pub use accounts::{CreatePluginAccountInput, PluginAccount, PluginAccountStore};
pub use manager::{PluginManager, CORE_DEFAULT_PLUGINS};
pub use manifest::{PluginAuthor, PluginManifest, CANONICAL_PLUGIN_SCHEMA_V1};
pub use marketplace::{get_curated_marketplace, MarketplacePlugin, PluginAuthConfig};
pub use mcp_config::{
    McpManifest, McpTransportType, PluginMcpServerConfig, CANONICAL_MCP_SCHEMA_V1,
};
pub use model::{
    CreateCustomPluginRequest, CustomSkillDefinition, InstallPluginRequest, InstalledPlugin,
    PluginMcpServerSummary, PluginSkillSummary, PluginSourceType, PluginStatus,
};
pub use skills_discovery::{discover_plugin_skills, DiscoveredPluginSkill};
