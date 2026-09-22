pub mod accounts;
pub mod branding;
pub mod manager;
pub mod manifest;
pub mod marketplace;
pub mod mcp_config;
pub mod model;
pub mod scaffold;
pub mod skills_discovery;

pub use accounts::{CreatePluginAccountInput, PluginAccount, PluginAccountStore};
pub use branding::{
    branding_from_extensions, build_branding_extension, confine_logo_path, is_allowed_logo_file_name,
    is_valid_brand_color, is_valid_logo_emoji, parse_logo_data_url, ARO_BRANDING_EXTENSION,
    LOGO_FILE_STEM, MAX_LOGO_BYTES, DecodedLogo, LogoKind, ResolvedBranding,
};
pub use manager::{PluginManager, CORE_DEFAULT_PLUGINS};
pub use manifest::{
    is_valid_plugin_name, PluginAuthor, PluginManifest, CANONICAL_PLUGIN_SCHEMA_V1,
};
pub use marketplace::{get_curated_marketplace, MarketplacePlugin, PluginAuthConfig};
pub use mcp_config::{
    McpManifest, McpTransportType, PluginMcpServerConfig, CANONICAL_MCP_SCHEMA_V1,
};
pub use model::{
    CreateCustomPluginRequest, CustomSkillDefinition, InstallPluginRequest, InstalledPlugin,
    PluginMcpServerSummary, PluginSkillSummary, PluginSourceType, PluginStatus,
};
pub use skills_discovery::{discover_plugin_skills, DiscoveredPluginSkill};
