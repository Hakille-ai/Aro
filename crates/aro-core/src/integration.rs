use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationKind {
    App,
    ModelProvider,
    McpServer,
    Webhook,
    Scheduler,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationAuthMode {
    None,
    #[serde(rename = "oauth2")]
    OAuth2,
    Oidc,
    ApiKey,
    ServiceAccount,
    LocalCredential,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationCredentialStore {
    ServerVault,
    DeviceKeyring,
    ExternalVault,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationStatus {
    NeedsAuth,
    NeedsConfig,
    Connecting,
    Connected,
    Error,
    Disabled,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationFieldType {
    Text,
    Password,
    Url,
    Select,
    Boolean,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConfigField {
    pub name: String,
    pub label: String,
    pub field_type: IntegrationFieldType,
    pub required: bool,
    pub secret: bool,
    pub placeholder: Option<String>,
    pub help_text: Option<String>,
    pub options: Vec<IntegrationFieldOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationFieldOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPermission {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationCapability {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub read_only: bool,
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationOAuthConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub revocation_url: Option<String>,
    pub redirect_path: String,
    pub default_scopes: Vec<String>,
    pub pkce_required: bool,
    pub oidc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationAuthConfig {
    pub mode: IntegrationAuthMode,
    pub credential_store: IntegrationCredentialStore,
    pub oauth: Option<IntegrationOAuthConfig>,
    pub config_fields: Vec<IntegrationConfigField>,
}

impl IntegrationAuthConfig {
    pub fn secret_field_names(&self) -> Vec<&str> {
        self.config_fields
            .iter()
            .filter(|field| field.secret)
            .map(|field| field.name.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationManifest {
    pub id: String,
    pub version: String,
    pub kind: IntegrationKind,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub icon: Option<String>,
    pub auth: IntegrationAuthConfig,
    pub permissions: Vec<IntegrationPermission>,
    pub capabilities: Vec<IntegrationCapability>,
}

impl IntegrationManifest {
    pub fn requires_redirect(&self) -> bool {
        matches!(
            self.auth.mode,
            IntegrationAuthMode::OAuth2 | IntegrationAuthMode::Oidc
        )
    }

    pub fn secret_field_names(&self) -> Vec<&str> {
        self.auth.secret_field_names()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationAccountProfile {
    pub external_account_id: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationCredentialRef {
    pub id: Uuid,
    pub store: IntegrationCredentialStore,
    pub key_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConnection {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub provider_id: String,
    pub manifest_version: String,
    pub status: IntegrationStatus,
    pub enabled: bool,
    pub granted_scopes: Vec<String>,
    pub account: IntegrationAccountProfile,
    pub credential: Option<IntegrationCredentialRef>,
    pub public_config: Value,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github_manifest() -> IntegrationManifest {
        IntegrationManifest {
            id: "github".to_string(),
            version: "2026-07-01".to_string(),
            kind: IntegrationKind::App,
            display_name: "GitHub".to_string(),
            description: "Connect repositories, issues, and pull requests.".to_string(),
            category: "development".to_string(),
            icon: Some("github".to_string()),
            auth: IntegrationAuthConfig {
                mode: IntegrationAuthMode::OAuth2,
                credential_store: IntegrationCredentialStore::ServerVault,
                oauth: Some(IntegrationOAuthConfig {
                    authorization_url: "https://github.com/login/oauth/authorize".to_string(),
                    token_url: "https://github.com/login/oauth/access_token".to_string(),
                    userinfo_url: Some("https://api.github.com/user".to_string()),
                    revocation_url: None,
                    redirect_path: "/integrations/github/oauth/callback".to_string(),
                    default_scopes: vec!["read:user".to_string()],
                    pkce_required: true,
                    oidc: false,
                }),
                config_fields: vec![IntegrationConfigField {
                    name: "pat".to_string(),
                    label: "Personal access token".to_string(),
                    field_type: IntegrationFieldType::Password,
                    required: false,
                    secret: true,
                    placeholder: Some("ghp_...".to_string()),
                    help_text: None,
                    options: Vec::new(),
                }],
            },
            permissions: vec![IntegrationPermission {
                id: "repo.read".to_string(),
                label: "Read repositories".to_string(),
                description: None,
                required: true,
                sensitive: false,
            }],
            capabilities: vec![IntegrationCapability {
                id: "repo.search".to_string(),
                label: "Search repositories".to_string(),
                description: None,
                read_only: true,
                permission_ids: vec!["repo.read".to_string()],
            }],
        }
    }

    #[test]
    fn oauth_manifest_serializes_with_stable_names() {
        let manifest = github_manifest();
        let json = serde_json::to_value(&manifest).expect("manifest json");

        assert_eq!(json["kind"], "app");
        assert_eq!(json["displayName"], "GitHub");
        assert_eq!(json["auth"]["mode"], "oauth2");
        assert_eq!(json["auth"]["credentialStore"], "server-vault");
        assert_eq!(json["auth"]["oauth"]["pkceRequired"], true);
    }

    #[test]
    fn manifest_declares_redirect_and_secret_fields() {
        let manifest = github_manifest();

        assert!(manifest.requires_redirect());
        assert_eq!(manifest.secret_field_names(), vec!["pat"]);
    }
}
