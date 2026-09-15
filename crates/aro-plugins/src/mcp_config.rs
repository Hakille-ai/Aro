use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const CANONICAL_MCP_SCHEMA_V1: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransportType {
    Stdio,
    #[serde(rename = "streamable-http")]
    StreamableHttp,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMcpServerConfig {
    #[serde(alias = "transportType", rename = "type")]
    pub transport_type: McpTransportType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpManifest {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, PluginMcpServerConfig>,
}

impl McpManifest {
    pub fn from_json_str(json_str: &str) -> Result<Self> {
        let manifest: McpManifest = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Invalid JSON syntax in mcp.json: {e}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        for (name, server) in &self.mcp_servers {
            if name.is_empty() {
                return Err(anyhow!("MCP server name cannot be empty"));
            }

            match server.transport_type {
                McpTransportType::Stdio => {
                    if server.command.is_none() {
                        return Err(anyhow!(
                            "MCP stdio server '{name}' must specify a 'command'"
                        ));
                    }
                }
                McpTransportType::StreamableHttp | McpTransportType::Sse => {
                    if server.url.is_none() {
                        return Err(anyhow!("MCP HTTP server '{name}' must specify a 'url'"));
                    }
                }
            }

            // Environment variable validation:
            // "Plugins must not define PLUGIN_ROOT or PLUGIN_DATA themselves within their env configuration"
            if server.env.contains_key("PLUGIN_ROOT") || server.env.contains_key("PLUGIN_DATA") {
                return Err(anyhow!(
                    "agent-plugins.org violation in server '{name}': plugins must not define PLUGIN_ROOT or PLUGIN_DATA in env"
                ));
            }
        }
        Ok(())
    }

    /// Resolves placeholders `${PLUGIN_ROOT}` and `${PLUGIN_DATA}` in args, env values, and cwd
    /// for a given server configuration.
    pub fn resolve_server_config(
        &self,
        server_name: &str,
        plugin_root: &Path,
        plugin_data: &Path,
    ) -> Result<PluginMcpServerConfig> {
        let server = self
            .mcp_servers
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{server_name}' not found in mcp.json"))?;

        let root_str = plugin_root.to_string_lossy();
        let data_str = plugin_data.to_string_lossy();

        let resolved_args = server
            .args
            .iter()
            .map(|arg| expand_placeholders(arg, &root_str, &data_str))
            .collect();

        let mut resolved_env = HashMap::new();
        // Automatically inject PLUGIN_ROOT and PLUGIN_DATA
        resolved_env.insert("PLUGIN_ROOT".to_string(), root_str.to_string());
        resolved_env.insert("PLUGIN_DATA".to_string(), data_str.to_string());

        for (k, v) in &server.env {
            resolved_env.insert(k.clone(), expand_placeholders(v, &root_str, &data_str));
        }

        let resolved_cwd = server
            .cwd
            .as_ref()
            .map(|cwd| expand_placeholders(cwd, &root_str, &data_str))
            .or_else(|| Some(root_str.to_string()));

        Ok(PluginMcpServerConfig {
            transport_type: server.transport_type,
            command: server.command.clone(),
            args: resolved_args,
            env: resolved_env,
            cwd: resolved_cwd,
            url: server.url.clone(),
            headers: server.headers.clone(),
        })
    }
}

fn expand_placeholders(input: &str, plugin_root: &str, plugin_data: &str) -> String {
    input
        .replace("${PLUGIN_ROOT}", plugin_root)
        .replace("${PLUGIN_DATA}", plugin_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_placeholders() {
        let json = r#"{
            "mcpServers": {
                "test-server": {
                    "type": "stdio",
                    "command": "node",
                    "args": ["${PLUGIN_ROOT}/index.js", "--data", "${PLUGIN_DATA}/cache"],
                    "env": {
                        "CONFIG_PATH": "${PLUGIN_ROOT}/config.json"
                    },
                    "cwd": "${PLUGIN_ROOT}"
                }
            }
        }"#;

        let manifest = McpManifest::from_json_str(json).unwrap();
        let root = Path::new("/opt/aro/plugins/test");
        let data = Path::new("/opt/aro/data/test");

        let resolved = manifest
            .resolve_server_config("test-server", root, data)
            .unwrap();
        assert_eq!(resolved.args[0], "/opt/aro/plugins/test/index.js");
        assert_eq!(resolved.args[2], "/opt/aro/data/test/cache");
        assert_eq!(
            resolved.env.get("CONFIG_PATH").unwrap(),
            "/opt/aro/plugins/test/config.json"
        );
        assert_eq!(
            resolved.env.get("PLUGIN_ROOT").unwrap(),
            "/opt/aro/plugins/test"
        );
    }

    #[test]
    fn test_rejects_plugin_root_in_env() {
        let json = r#"{
            "mcpServers": {
                "bad-server": {
                    "type": "stdio",
                    "command": "node",
                    "env": {
                        "PLUGIN_ROOT": "/custom/path"
                    }
                }
            }
        }"#;

        assert!(McpManifest::from_json_str(json).is_err());
    }
}
