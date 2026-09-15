use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub const CANONICAL_PLUGIN_SCHEMA_V1: &str =
    "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PluginAuthor {
    String(String),
    Object {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

impl PluginAuthor {
    pub fn display_name(&self) -> &str {
        match self {
            PluginAuthor::String(s) => s.as_str(),
            PluginAuthor::Object { name, .. } => name.as_str(),
        }
    }
}

/// Manifest structure for `plugin.json` according to https://agent-plugins.org/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(rename = "$schema", default = "default_schema")]
    pub schema: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<PluginAuthor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, Value>,
}

fn default_schema() -> String {
    CANONICAL_PLUGIN_SCHEMA_V1.to_string()
}

impl PluginManifest {
    /// Parse and validate `plugin.json` from a JSON string, enforcing the official
    /// agent-plugins.org specification.
    pub fn from_json_str(json_str: &str) -> Result<Self> {
        let val: Value = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Invalid JSON syntax in plugin.json: {e}"))?;

        let obj = val
            .as_object()
            .ok_or_else(|| anyhow!("plugin.json must be a JSON object"))?;

        // Closed schema check: Disallow unauthorized top-level keys like "mcpServers", "skills", "commands"
        let allowed_keys = [
            "$schema",
            "name",
            "version",
            "description",
            "author",
            "homepage",
            "repository",
            "license",
            "keywords",
            "extensions",
        ];

        for key in obj.keys() {
            if !allowed_keys.contains(&key.as_str()) {
                tracing::warn!(
                    key = %key,
                    "Reporting and ignoring unknown top-level property '{key}' in plugin.json per agent-plugins.org §5.2"
                );
            }
        }

        let manifest: PluginManifest = serde_json::from_value(val)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates compliance with agent-plugins.org rules
    pub fn validate(&self) -> Result<()> {
        // Name validation per §5.5 (1-64 chars, a-z0-9-., alphanumeric start/end, no -- or ..)
        if !is_valid_plugin_name(&self.name) {
            return Err(anyhow!(
                "Invalid plugin name '{}'. Plugin names must be 1-64 chars, start/end with alphanumeric, contain only [a-z0-9-.], with no consecutive '--' or '..' per agent-plugins.org §5.5",
                self.name
            ));
        }

        // Schema validation: should match agent-plugins.org schema
        if !self.schema.is_empty() && !self.schema.contains("agent-plugins.org") {
            tracing::warn!(
                schema = %self.schema,
                "Plugin manifest $schema does not point to agent-plugins.org"
            );
        }

        // Extensions validation: keys should use reverse-domain notation
        for key in self.extensions.keys() {
            if !key.contains('.') {
                tracing::debug!(
                    key = %key,
                    "Extension key should ideally follow reverse-domain notation (e.g. 'com.example.client')"
                );
            }
        }

        Ok(())
    }
}

/// Validates plugin names per agent-plugins.org §5.5
fn is_valid_plugin_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 64 {
        return false;
    }
    let first = match s.chars().next() {
        Some(c) => c,
        None => return false,
    };
    let last = match s.chars().last() {
        Some(c) => c,
        None => return false,
    };
    if (!first.is_ascii_lowercase() && !first.is_ascii_digit())
        || (!last.is_ascii_lowercase() && !last.is_ascii_digit())
    {
        return false;
    }
    if s.contains("--") || s.contains("..") {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_manifest() {
        let json = r#"{
            "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
            "name": "example-plugin",
            "version": "1.0.0",
            "description": "An example plugin",
            "keywords": ["agent", "test"]
        }"#;

        let manifest = PluginManifest::from_json_str(json).unwrap();
        assert_eq!(manifest.name, "example-plugin");
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_ignores_unknown_top_level_fields() {
        let json = r#"{
            "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
            "name": "plugin-with-extra",
            "unknownField": {}
        }"#;

        let manifest = PluginManifest::from_json_str(json).expect("should succeed per §5.2");
        assert_eq!(manifest.name, "plugin-with-extra");
    }

    #[test]
    fn test_plugin_name_with_dots() {
        let json = r#"{
            "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
            "name": "acme.tools"
        }"#;

        let manifest =
            PluginManifest::from_json_str(json).expect("names with dots are valid per §5.5");
        assert_eq!(manifest.name, "acme.tools");
    }

    #[test]
    fn test_rejects_uppercase_name() {
        let json = r#"{
            "name": "BadPlugin"
        }"#;

        assert!(PluginManifest::from_json_str(json).is_err());
    }
}
