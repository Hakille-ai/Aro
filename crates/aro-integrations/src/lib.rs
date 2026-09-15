//! Typed connector SDK and protocol-neutral integration domain.
//!
//! Connectors never persist credentials themselves. They exchange provider-specific payloads
//! for [`CredentialBundle`] values that the application encrypts before opening a DB transaction.

use std::{collections::BTreeMap, fmt, net::IpAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use reqwest::{redirect::Policy as RedirectPolicy, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256, Sha384, Sha512};
use tokio::net::lookup_host;
use url::{Host, Url};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    AuthorizationCodePkce,
    OpenIdConnect,
    DeviceCode,
    AppInstallation,
    ApiKey,
    PersonalAccessToken,
    ServiceAccount,
    ClientCredentials,
    LocalCredential,
    ExternalVault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerType {
    User,
    Team,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRef {
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
    pub user_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    Pending,
    Active,
    ReauthorizationRequired,
    Disconnecting,
    Revoked,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Degraded,
    ProviderUnavailable,
    CredentialsExpired,
    PermissionsInsufficient,
    InvalidCredentials,
    SyncError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationAttemptStatus {
    Pending,
    Processing,
    Authorized,
    Denied,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorPermission {
    pub id: String,
    pub label: String,
    pub description: String,
    pub required: bool,
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCapability {
    pub id: String,
    pub label: String,
    pub description: String,
    pub read_only: bool,
    pub permission_ids: Vec<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDescriptor {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub icon: Option<String>,
    pub auth_methods: Vec<AuthMethod>,
    pub permissions: Vec<ConnectorPermission>,
    pub capabilities: Vec<ConnectorCapability>,
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub public_config_schema: Value,
}

impl ConnectorDescriptor {
    pub fn validate(&self) -> Result<(), ConnectorError> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        {
            return Err(ConnectorError::misconfigured("invalid connector id"));
        }
        if self.version.trim().is_empty() || self.auth_methods.is_empty() {
            return Err(ConnectorError::misconfigured(
                "connector version and auth methods are required",
            ));
        }
        for capability in &self.capabilities {
            if capability.permission_ids.iter().any(|permission_id| {
                !self
                    .permissions
                    .iter()
                    .any(|permission| &permission.id == permission_id)
            }) {
                return Err(ConnectorError::misconfigured(format!(
                    "capability {} references an unknown permission",
                    capability.id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizationContext {
    pub attempt_id: Uuid,
    pub organization_id: Uuid,
    pub actor_user_id: Uuid,
    pub owner: OwnerRef,
    pub method: AuthMethod,
    pub capabilities: Vec<String>,
    pub requested_scopes: Vec<String>,
    pub callback_uri: Url,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: Zeroizing<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthorizationInteraction {
    BrowserRedirect {
        url: String,
    },
    DeviceCode {
        verification_uri: String,
        verification_uri_complete: Option<String>,
        user_code: String,
        expires_at: DateTime<Utc>,
        interval_seconds: u64,
    },
    CredentialForm {
        schema: Value,
    },
}

#[derive(Clone)]
pub struct CredentialBundle {
    pub credential_type: String,
    pub secret: Zeroizing<String>,
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl fmt::Debug for CredentialBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialBundle")
            .field("credential_type", &self.credential_type)
            .field("secret", &"[REDACTED]")
            .field("granted_scopes", &self.granted_scopes)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalIdentity {
    pub external_account_id: String,
    pub display_name: Option<String>,
    pub realm: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub status: HealthStatus,
    pub checked_at: DateTime<Utc>,
    pub error_code: Option<ConnectorErrorCode>,
    pub retry_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CallbackPayload {
    pub code: String,
    pub state: String,
    pub issuer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VerifiedWebhook {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: Option<DateTime<Utc>>,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorErrorCode {
    AccessDenied,
    InvalidGrant,
    InteractionRequired,
    ConsentRequired,
    InsufficientScope,
    CredentialsExpired,
    RateLimited,
    ProviderUnavailable,
    Timeout,
    Misconfigured,
    ProtocolError,
    Revoked,
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct ConnectorError {
    pub code: ConnectorErrorCode,
    pub message: String,
    pub transient: bool,
    pub retry_after: Option<Duration>,
}

impl ConnectorError {
    pub fn permanent(code: ConnectorErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            transient: false,
            retry_after: None,
        }
    }

    pub fn transient(code: ConnectorErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            transient: true,
            retry_after: None,
        }
    }

    pub fn misconfigured(message: impl Into<String>) -> Self {
        Self::permanent(ConnectorErrorCode::Misconfigured, message)
    }
}

#[async_trait]
pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;
    async fn begin_authorization(
        &self,
        context: &AuthorizationContext,
    ) -> Result<AuthorizationInteraction, ConnectorError>;
    async fn complete_authorization(
        &self,
        context: &AuthorizationContext,
        callback: CallbackPayload,
    ) -> Result<CredentialBundle, ConnectorError>;
    async fn refresh_credentials(
        &self,
        credential: &CredentialBundle,
    ) -> Result<CredentialBundle, ConnectorError>;
    async fn fetch_external_identity(
        &self,
        credential: &CredentialBundle,
    ) -> Result<ExternalIdentity, ConnectorError>;
    async fn check_health(
        &self,
        credential: &CredentialBundle,
    ) -> Result<HealthReport, ConnectorError>;
    async fn revoke_credentials(&self, credential: CredentialBundle) -> Result<(), ConnectorError>;
    async fn register_webhooks(
        &self,
        _credential: &CredentialBundle,
    ) -> Result<(), ConnectorError> {
        Ok(())
    }
    async fn verify_webhook(
        &self,
        _headers: &BTreeMap<String, String>,
        _body: &[u8],
    ) -> Result<VerifiedWebhook, ConnectorError> {
        Err(ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "webhooks are not supported",
        ))
    }
    async fn unregister_webhooks(
        &self,
        _credential: &CredentialBundle,
    ) -> Result<(), ConnectorError> {
        Ok(())
    }
    fn normalize_error(&self, error: ConnectorError) -> ConnectorError {
        error
    }
}

#[derive(Debug)]
pub struct CertificationFixture {
    pub authorization_context: AuthorizationContext,
    pub callback: CallbackPayload,
    pub webhook_headers: BTreeMap<String, String>,
    pub webhook_body: Vec<u8>,
    pub exercise_webhooks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificationReport {
    pub connector_id: String,
    pub connector_version: String,
    pub checks: Vec<String>,
}

/// Contract harness run against provider-owned development fixtures before a manifest can be
/// marked `certified`. It deliberately exercises the complete trait rather than accepting a
/// descriptor-only registration.
pub struct ConnectorCertificationHarness;

impl ConnectorCertificationHarness {
    pub async fn run(
        connector: &dyn Connector,
        fixture: CertificationFixture,
    ) -> Result<CertificationReport, ConnectorError> {
        connector.descriptor().validate()?;
        let mut checks = vec!["descriptor".to_string()];
        let interaction = connector
            .begin_authorization(&fixture.authorization_context)
            .await?;
        match interaction {
            AuthorizationInteraction::BrowserRedirect { ref url } => {
                let parsed = Url::parse(url).map_err(|_| {
                    ConnectorError::permanent(
                        ConnectorErrorCode::ProtocolError,
                        "certification authorization URL is invalid",
                    )
                })?;
                if parsed.scheme() != "https" {
                    return Err(ConnectorError::permanent(
                        ConnectorErrorCode::ProtocolError,
                        "certification authorization URL is not HTTPS",
                    ));
                }
            }
            AuthorizationInteraction::DeviceCode {
                interval_seconds,
                expires_at,
                ..
            } if interval_seconds > 0 && expires_at > Utc::now() => {}
            AuthorizationInteraction::CredentialForm { ref schema } if schema.is_object() => {}
            _ => {
                return Err(ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "certification authorization interaction is invalid",
                ));
            }
        }
        checks.push("begin_authorization".into());
        let credential = connector
            .complete_authorization(&fixture.authorization_context, fixture.callback)
            .await?;
        if credential.secret.is_empty() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "certification returned an empty credential",
            ));
        }
        checks.push("complete_authorization".into());
        let identity = connector.fetch_external_identity(&credential).await?;
        if identity.external_account_id.trim().is_empty() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "certification returned an empty external identity",
            ));
        }
        checks.push("external_identity".into());
        let health = connector.check_health(&credential).await?;
        if health.checked_at > Utc::now() + chrono::Duration::minutes(1) {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "certification health timestamp is invalid",
            ));
        }
        checks.push("health".into());
        let refreshed = connector.refresh_credentials(&credential).await?;
        checks.push("refresh".into());
        if fixture.exercise_webhooks {
            connector.register_webhooks(&refreshed).await?;
            connector
                .verify_webhook(&fixture.webhook_headers, &fixture.webhook_body)
                .await?;
            connector.unregister_webhooks(&refreshed).await?;
            checks.push("webhooks".into());
        }
        connector.revoke_credentials(refreshed).await?;
        checks.push("revocation".into());
        let normalized = connector.normalize_error(ConnectorError::transient(
            ConnectorErrorCode::RateLimited,
            "certification sentinel",
        ));
        if normalized.code != ConnectorErrorCode::RateLimited {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "connector error normalization changed the category",
            ));
        }
        checks.push("error_normalization".into());
        Ok(CertificationReport {
            connector_id: connector.descriptor().id.clone(),
            connector_version: connector.descriptor().version.clone(),
            checks,
        })
    }
}

#[derive(Default, Clone)]
pub struct ConnectorRegistry {
    connectors: BTreeMap<String, Arc<dyn Connector>>,
}

impl ConnectorRegistry {
    pub fn register(&mut self, connector: Arc<dyn Connector>) -> Result<(), ConnectorError> {
        connector.descriptor().validate()?;
        let id = connector.descriptor().id.clone();
        if self.connectors.insert(id.clone(), connector).is_some() {
            return Err(ConnectorError::misconfigured(format!(
                "connector {id} is already registered"
            )));
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(id).cloned()
    }

    pub fn descriptors(&self) -> Vec<ConnectorDescriptor> {
        self.connectors
            .values()
            .map(|connector| connector.descriptor().clone())
            .collect()
    }
}

#[derive(Clone)]
pub struct SafeHttpClient {
    allowed_hosts: Vec<String>,
    timeout: Duration,
}

impl SafeHttpClient {
    pub fn new(allowed_hosts: Vec<String>, timeout: Duration) -> Result<Self, ConnectorError> {
        if allowed_hosts.is_empty() {
            return Err(ConnectorError::misconfigured(
                "allowed hosts cannot be empty",
            ));
        }
        Ok(Self {
            allowed_hosts,
            timeout,
        })
    }

    pub async fn client_for_url(&self, url: &Url) -> Result<Client, ConnectorError> {
        let addresses = self.validated_addresses(url).await?;
        let mut builder = Client::builder()
            .redirect(RedirectPolicy::none())
            .timeout(self.timeout)
            .https_only(true);
        if matches!(url.host(), Some(Host::Domain(_))) {
            let host = url
                .host_str()
                .ok_or_else(|| ConnectorError::misconfigured("connector URL has no host"))?;
            builder = builder.resolve_to_addrs(host, &addresses);
        }
        builder
            .build()
            .map_err(|_| ConnectorError::misconfigured("unable to build connector HTTP client"))
    }

    pub async fn validate_url(&self, url: &Url) -> Result<(), ConnectorError> {
        self.validated_addresses(url).await.map(|_| ())
    }

    async fn validated_addresses(
        &self,
        url: &Url,
    ) -> Result<Vec<std::net::SocketAddr>, ConnectorError> {
        if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
            return Err(ConnectorError::misconfigured(
                "connector URL must be a credential-free HTTPS URL",
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| ConnectorError::misconfigured("connector URL has no host"))?;
        if !self.allowed_hosts.iter().any(|allowed| host == allowed) {
            return Err(ConnectorError::misconfigured(
                "connector host is not allowlisted",
            ));
        }
        let addresses = match url.host() {
            Some(Host::Ipv4(ip)) if !is_public_ip(IpAddr::V4(ip)) => {
                return Err(ConnectorError::misconfigured(
                    "private connector IP is forbidden",
                ));
            }
            Some(Host::Ipv6(ip)) if !is_public_ip(IpAddr::V6(ip)) => {
                return Err(ConnectorError::misconfigured(
                    "private connector IP is forbidden",
                ));
            }
            Some(Host::Domain(_)) => {
                let port = url.port_or_known_default().unwrap_or(443);
                let addresses = lookup_host((host, port)).await.map_err(|_| {
                    ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "DNS lookup failed",
                    )
                })?;
                let addresses = addresses.collect::<Vec<_>>();
                if addresses.is_empty() {
                    return Err(ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "DNS lookup returned no address",
                    ));
                }
                if addresses.iter().any(|address| !is_public_ip(address.ip())) {
                    return Err(ConnectorError::misconfigured(
                        "connector DNS resolves to a private address",
                    ));
                }
                addresses
            }
            Some(Host::Ipv4(ip)) => vec![std::net::SocketAddr::new(
                IpAddr::V4(ip),
                url.port_or_known_default().unwrap_or(443),
            )],
            Some(Host::Ipv6(ip)) => vec![std::net::SocketAddr::new(
                IpAddr::V6(ip),
                url.port_or_known_default().unwrap_or(443),
            )],
            None => return Err(ConnectorError::misconfigured("connector URL has no host")),
        };
        Ok(addresses)
    }
}

pub fn pkce_challenge(verifier: &str) -> Result<String, ConnectorError> {
    if !(43..=128).contains(&verifier.len()) || !verifier.bytes().all(is_pkce_byte) {
        return Err(ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "PKCE verifier must contain 43-128 unreserved characters",
        ));
    }
    Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub revocation_url: Option<String>,
    pub issuer: Option<String>,
    #[serde(default)]
    pub extra_authorization_params: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_token_params: BTreeMap<String, String>,
}

#[derive(Clone)]
pub struct OAuthClientProfile {
    pub client_id: String,
    pub client_secret: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub struct OAuthExchangeResult {
    pub credential: CredentialBundle,
    pub identity: Option<ExternalIdentity>,
}

#[derive(Clone)]
pub struct OAuthProtocolEngine {
    config: OAuthProviderConfig,
    http: SafeHttpClient,
}

impl OAuthProtocolEngine {
    pub fn new(
        config: OAuthProviderConfig,
        allowed_hosts: Vec<String>,
    ) -> Result<Self, ConnectorError> {
        Ok(Self {
            config,
            http: SafeHttpClient::new(allowed_hosts, Duration::from_secs(15))?,
        })
    }

    pub async fn authorization_url(
        &self,
        context: &AuthorizationContext,
        profile: &OAuthClientProfile,
    ) -> Result<Url, ConnectorError> {
        let mut url = Url::parse(&self.config.authorization_url)
            .map_err(|_| ConnectorError::misconfigured("authorization URL is invalid"))?;
        self.http.validate_url(&url).await?;
        let challenge = pkce_challenge(context.pkce_verifier.as_str())?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &profile.client_id)
                .append_pair("redirect_uri", context.callback_uri.as_str())
                .append_pair("state", &context.state)
                .append_pair("scope", &context.requested_scopes.join(" "))
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "S256");
            if self.config.issuer.is_some() {
                query.append_pair("nonce", &context.nonce);
            }
            for (name, value) in &self.config.extra_authorization_params {
                if !reserved_authorization_parameter(name) {
                    query.append_pair(name, value);
                }
            }
        }
        Ok(url)
    }

    pub async fn exchange_code(
        &self,
        context: &AuthorizationContext,
        profile: &OAuthClientProfile,
        code: &str,
        callback_issuer: Option<&str>,
    ) -> Result<OAuthExchangeResult, ConnectorError> {
        if code.trim().is_empty() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "authorization code is missing",
            ));
        }
        if let (Some(expected), Some(actual)) = (self.config.issuer.as_deref(), callback_issuer) {
            if expected != actual {
                return Err(ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "authorization issuer mismatch",
                ));
            }
        }
        let token_url = Url::parse(&self.config.token_url)
            .map_err(|_| ConnectorError::misconfigured("token URL is invalid"))?;
        let client = self.http.client_for_url(&token_url).await?;
        let mut form = vec![
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("client_id".to_string(), profile.client_id.clone()),
            ("code".to_string(), code.to_string()),
            ("redirect_uri".to_string(), context.callback_uri.to_string()),
            (
                "code_verifier".to_string(),
                context.pkce_verifier.to_string(),
            ),
        ];
        if let Some(secret) = profile.client_secret.as_ref() {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        for (name, value) in &self.config.extra_token_params {
            if !reserved_token_parameter(name) {
                form.push((name.clone(), value.clone()));
            }
        }
        let response = client
            .post(token_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ConnectorError::transient(
                        ConnectorErrorCode::Timeout,
                        "token endpoint timed out",
                    )
                } else {
                    ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "token endpoint is unavailable",
                    )
                }
            })?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ConnectorError::transient(
                ConnectorErrorCode::RateLimited,
                "token endpoint rate limited the request",
            ));
        }
        if !response.status().is_success() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::InvalidGrant,
                "authorization code exchange was rejected",
            ));
        }
        let bytes = response.bytes().await.map_err(|_| {
            ConnectorError::transient(
                ConnectorErrorCode::ProviderUnavailable,
                "token response could not be read",
            )
        })?;
        if bytes.len() > 1024 * 1024 {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "token response exceeds one MiB",
            ));
        }
        let token: OAuthTokenResponse = serde_json::from_slice(&bytes).map_err(|_| {
            ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "token response is invalid",
            )
        })?;
        if token.access_token.trim().is_empty() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "token response has no access token",
            ));
        }
        let granted_scopes = token
            .scope
            .as_deref()
            .map(parse_scope)
            .unwrap_or_else(|| context.requested_scopes.clone());
        let expires_at = token
            .expires_in
            .and_then(|seconds| i64::try_from(seconds).ok())
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));
        let credential = credential_from_oauth_token(
            &token,
            granted_scopes.clone(),
            expires_at,
            self.config.issuer.is_some(),
        )?;
        if context
            .requested_scopes
            .iter()
            .any(|scope| !granted_scopes.iter().any(|granted| granted == scope))
        {
            let _ = self.revoke_credential(profile, &credential).await;
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::InsufficientScope,
                "provider granted fewer scopes than required",
            ));
        }
        let identity = if let Some(issuer) = self.config.issuer.as_deref() {
            let Some(id_token) = token.id_token.as_deref() else {
                let _ = self.revoke_credential(profile, &credential).await;
                return Err(ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "OIDC token response has no ID token",
                ));
            };
            match self
                .validate_id_token(
                    id_token,
                    &token.access_token,
                    issuer,
                    &profile.client_id,
                    &context.nonce,
                )
                .await
            {
                Ok(identity) => Some(identity),
                Err(error) => {
                    let _ = self.revoke_credential(profile, &credential).await;
                    return Err(error);
                }
            }
        } else {
            None
        };
        Ok(OAuthExchangeResult {
            credential,
            identity,
        })
    }

    /// Best-effort compensation used when a provider issued a token but local atomic
    /// finalization failed. No token value is ever included in the returned error.
    pub async fn revoke_credential(
        &self,
        profile: &OAuthClientProfile,
        credential: &CredentialBundle,
    ) -> Result<(), ConnectorError> {
        let revocation_url = self.config.revocation_url.as_deref().ok_or_else(|| {
            ConnectorError::misconfigured("connector has no token revocation endpoint")
        })?;
        let revocation_url = Url::parse(revocation_url)
            .map_err(|_| ConnectorError::misconfigured("revocation URL is invalid"))?;
        let client = self.http.client_for_url(&revocation_url).await?;
        let secret: Value = serde_json::from_str(credential.secret.as_str()).map_err(|_| {
            ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "credential payload cannot be revoked",
            )
        })?;
        let token = secret
            .get("refreshToken")
            .and_then(Value::as_str)
            .or_else(|| secret.get("accessToken").and_then(Value::as_str))
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "credential payload has no revocable token",
                )
            })?;
        let mut form = vec![
            ("token".to_string(), token.to_string()),
            ("client_id".to_string(), profile.client_id.clone()),
        ];
        if let Some(secret) = profile.client_secret.as_ref() {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        let response = client
            .post(revocation_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ConnectorError::transient(
                        ConnectorErrorCode::Timeout,
                        "revocation endpoint timed out",
                    )
                } else {
                    ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "revocation endpoint is unavailable",
                    )
                }
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ConnectorError::transient(
                ConnectorErrorCode::ProviderUnavailable,
                "provider did not confirm token revocation",
            ))
        }
    }

    async fn validate_id_token(
        &self,
        id_token: &str,
        access_token: &str,
        expected_issuer: &str,
        client_id: &str,
        nonce: &str,
    ) -> Result<ExternalIdentity, ConnectorError> {
        let discovery_url = Url::parse(&format!(
            "{}/.well-known/openid-configuration",
            expected_issuer.trim_end_matches('/')
        ))
        .map_err(|_| ConnectorError::misconfigured("OIDC issuer is invalid"))?;
        let discovery: OidcDiscovery = fetch_limited_json(&self.http, discovery_url).await?;
        if discovery.issuer != expected_issuer {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "OIDC discovery issuer mismatch",
            ));
        }
        let jwks_url = Url::parse(&discovery.jwks_uri)
            .map_err(|_| ConnectorError::misconfigured("OIDC JWKS URI is invalid"))?;
        let jwks: JwkSet = fetch_limited_json(&self.http, jwks_url).await?;
        let header = decode_header(id_token).map_err(|_| {
            ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "ID token header is invalid",
            )
        })?;
        if !matches!(
            header.alg,
            Algorithm::RS256
                | Algorithm::RS384
                | Algorithm::RS512
                | Algorithm::PS256
                | Algorithm::PS384
                | Algorithm::PS512
                | Algorithm::ES256
                | Algorithm::ES384
                | Algorithm::EdDSA
        ) {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "ID token algorithm is not allowed",
            ));
        }
        let kid = header.kid.as_deref().ok_or_else(|| {
            ConnectorError::permanent(ConnectorErrorCode::ProtocolError, "ID token has no key id")
        })?;
        let jwk = jwks.find(kid).ok_or_else(|| {
            ConnectorError::permanent(ConnectorErrorCode::ProtocolError, "ID token key is unknown")
        })?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| {
            ConnectorError::permanent(ConnectorErrorCode::ProtocolError, "ID token key is invalid")
        })?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[expected_issuer]);
        validation.set_audience(&[client_id]);
        validation.leeway = 60;
        let claims = decode::<Value>(id_token, &key, &validation)
            .map_err(|_| {
                ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "ID token validation failed",
                )
            })?
            .claims;
        if claims.get("nonce").and_then(Value::as_str) != Some(nonce) {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "ID token nonce mismatch",
            ));
        }
        if claims
            .get("iat")
            .and_then(Value::as_i64)
            .is_none_or(|issued_at| issued_at > Utc::now().timestamp() + 60)
        {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "ID token issued-at time is invalid",
            ));
        }
        let audience_count = claims
            .get("aud")
            .and_then(Value::as_array)
            .map_or(1, Vec::len);
        let authorized_party = claims.get("azp").and_then(Value::as_str);
        if authorized_party.is_some_and(|party| party != client_id)
            || (audience_count > 1 && authorized_party != Some(client_id))
        {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "ID token authorized party mismatch",
            ));
        }
        if let Some(expected_hash) = claims.get("at_hash").and_then(Value::as_str) {
            if access_token_hash(access_token, header.alg) != Some(expected_hash.to_string()) {
                return Err(ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "ID token access-token hash mismatch",
                ));
            }
        }
        let subject = claims.get("sub").and_then(Value::as_str).ok_or_else(|| {
            ConnectorError::permanent(ConnectorErrorCode::ProtocolError, "ID token has no subject")
        })?;
        Ok(ExternalIdentity {
            external_account_id: subject.to_string(),
            display_name: claims
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string),
            realm: Some(expected_issuer.to_string()),
            avatar_url: claims
                .get("picture")
                .and_then(Value::as_str)
                .map(str::to_string),
            details: serde_json::json!({}),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProviderConfig {
    pub device_authorization_url: String,
    pub token_url: String,
}

pub struct DeviceAuthorization {
    pub device_code: Zeroizing<String>,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub interval: Duration,
    pub next_poll_at: DateTime<Utc>,
}

impl fmt::Debug for DeviceAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceAuthorization")
            .field("device_code", &"[REDACTED]")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_at", &self.expires_at)
            .field("interval", &self.interval)
            .field("next_poll_at", &self.next_poll_at)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum DevicePollResult {
    Pending { next_poll_at: DateTime<Utc> },
    Authorized(CredentialBundle),
}

#[derive(Clone)]
pub struct DeviceProtocolEngine {
    config: DeviceProviderConfig,
    http: SafeHttpClient,
}

impl DeviceProtocolEngine {
    pub fn new(
        config: DeviceProviderConfig,
        allowed_hosts: Vec<String>,
    ) -> Result<Self, ConnectorError> {
        Ok(Self {
            config,
            http: SafeHttpClient::new(allowed_hosts, Duration::from_secs(15))?,
        })
    }

    pub async fn begin(
        &self,
        client_id: &str,
        scopes: &[String],
    ) -> Result<DeviceAuthorization, ConnectorError> {
        let endpoint = Url::parse(&self.config.device_authorization_url)
            .map_err(|_| ConnectorError::misconfigured("device authorization URL is invalid"))?;
        let client = self.http.client_for_url(&endpoint).await?;
        let scope = scopes.join(" ");
        let response = client
            .post(endpoint)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&[("client_id", client_id), ("scope", scope.as_str())])
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ConnectorError::transient(
                        ConnectorErrorCode::Timeout,
                        "device authorization endpoint timed out",
                    )
                } else {
                    ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "device authorization endpoint is unavailable",
                    )
                }
            })?;
        if !response.status().is_success() {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "device authorization request was rejected",
            ));
        }
        let payload: DeviceAuthorizationResponse = limited_response_json(response).await?;
        if payload.device_code.is_empty()
            || payload.user_code.is_empty()
            || payload.expires_in == 0
            || payload.verification_uri.is_empty()
        {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "device authorization response is incomplete",
            ));
        }
        let verification = Url::parse(&payload.verification_uri).map_err(|_| {
            ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "device verification URI is invalid",
            )
        })?;
        self.http.validate_url(&verification).await?;
        let interval = Duration::from_secs(payload.interval.unwrap_or(5).max(1));
        let expires_at = Utc::now()
            + chrono::Duration::seconds(i64::try_from(payload.expires_in).unwrap_or(i64::MAX));
        Ok(DeviceAuthorization {
            device_code: Zeroizing::new(payload.device_code),
            user_code: payload.user_code,
            verification_uri: payload.verification_uri,
            verification_uri_complete: payload.verification_uri_complete,
            expires_at,
            interval,
            next_poll_at: Utc::now()
                + chrono::Duration::from_std(interval)
                    .unwrap_or_else(|_| chrono::Duration::seconds(5)),
        })
    }

    pub async fn poll(
        &self,
        profile: &OAuthClientProfile,
        authorization: &mut DeviceAuthorization,
    ) -> Result<DevicePollResult, ConnectorError> {
        let now = Utc::now();
        if now >= authorization.expires_at {
            return Err(ConnectorError::permanent(
                ConnectorErrorCode::CredentialsExpired,
                "device authorization expired",
            ));
        }
        if now < authorization.next_poll_at {
            return Ok(DevicePollResult::Pending {
                next_poll_at: authorization.next_poll_at,
            });
        }
        let endpoint = Url::parse(&self.config.token_url)
            .map_err(|_| ConnectorError::misconfigured("device token URL is invalid"))?;
        let client = self.http.client_for_url(&endpoint).await?;
        let mut form = vec![
            (
                "grant_type".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ),
            ("client_id".to_string(), profile.client_id.clone()),
            (
                "device_code".to_string(),
                authorization.device_code.to_string(),
            ),
        ];
        if let Some(secret) = profile.client_secret.as_ref() {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        let response = client
            .post(endpoint)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ConnectorError::transient(
                        ConnectorErrorCode::Timeout,
                        "device token endpoint timed out",
                    )
                } else {
                    ConnectorError::transient(
                        ConnectorErrorCode::ProviderUnavailable,
                        "device token endpoint is unavailable",
                    )
                }
            })?;
        let status = response.status();
        let bytes = limited_response_bytes(response).await?;
        if status.is_success() {
            let token: OAuthTokenResponse = serde_json::from_slice(&bytes).map_err(|_| {
                ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "device token response is invalid",
                )
            })?;
            if token.access_token.is_empty() {
                return Err(ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "device token response has no access token",
                ));
            }
            let granted_scopes = token.scope.as_deref().map(parse_scope).unwrap_or_default();
            let expires_at = token
                .expires_in
                .and_then(|seconds| i64::try_from(seconds).ok())
                .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));
            let secret = serde_json::to_string(&serde_json::json!({
                "accessToken": token.access_token,
                "refreshToken": token.refresh_token,
                "tokenType": token.token_type,
                "idToken": token.id_token,
            }))
            .map_err(|_| {
                ConnectorError::permanent(
                    ConnectorErrorCode::ProtocolError,
                    "device token serialization failed",
                )
            })?;
            return Ok(DevicePollResult::Authorized(CredentialBundle {
                credential_type: "device_code".into(),
                secret: Zeroizing::new(secret),
                granted_scopes,
                expires_at,
            }));
        }
        let error: DeviceTokenError = serde_json::from_slice(&bytes).unwrap_or(DeviceTokenError {
            error: "protocol_error".into(),
        });
        match error.error.as_str() {
            "authorization_pending" => {
                authorization.next_poll_at = now
                    + chrono::Duration::from_std(authorization.interval)
                        .unwrap_or_else(|_| chrono::Duration::seconds(5));
                Ok(DevicePollResult::Pending {
                    next_poll_at: authorization.next_poll_at,
                })
            }
            "slow_down" => {
                authorization.interval += Duration::from_secs(5);
                authorization.next_poll_at = now
                    + chrono::Duration::from_std(authorization.interval)
                        .unwrap_or_else(|_| chrono::Duration::seconds(10));
                Ok(DevicePollResult::Pending {
                    next_poll_at: authorization.next_poll_at,
                })
            }
            "access_denied" => Err(ConnectorError::permanent(
                ConnectorErrorCode::AccessDenied,
                "device authorization was denied",
            )),
            "expired_token" => Err(ConnectorError::permanent(
                ConnectorErrorCode::CredentialsExpired,
                "device authorization expired",
            )),
            _ => Err(ConnectorError::permanent(
                ConnectorErrorCode::ProtocolError,
                "device token endpoint returned an invalid error",
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenError {
    error: String,
}

async fn limited_response_bytes(response: reqwest::Response) -> Result<Vec<u8>, ConnectorError> {
    let bytes = response.bytes().await.map_err(|_| {
        ConnectorError::transient(
            ConnectorErrorCode::ProviderUnavailable,
            "provider response could not be read",
        )
    })?;
    if bytes.len() > 1024 * 1024 {
        return Err(ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "provider response exceeds one MiB",
        ));
    }
    Ok(bytes.to_vec())
}

async fn limited_response_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, ConnectorError> {
    let bytes = limited_response_bytes(response).await?;
    serde_json::from_slice(&bytes).map_err(|_| {
        ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "provider response is invalid",
        )
    })
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    id_token: Option<String>,
}

fn credential_from_oauth_token(
    token: &OAuthTokenResponse,
    granted_scopes: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
    is_oidc: bool,
) -> Result<CredentialBundle, ConnectorError> {
    let secret = serde_json::to_string(&serde_json::json!({
        "accessToken": &token.access_token,
        "refreshToken": &token.refresh_token,
        "tokenType": &token.token_type,
        "idToken": &token.id_token,
    }))
    .map_err(|_| {
        ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "token serialization failed",
        )
    })?;
    Ok(CredentialBundle {
        credential_type: if is_oidc { "oidc" } else { "oauth2" }.into(),
        secret: Zeroizing::new(secret),
        granted_scopes,
        expires_at,
    })
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    issuer: String,
    jwks_uri: String,
}

async fn fetch_limited_json<T: for<'de> Deserialize<'de>>(
    http: &SafeHttpClient,
    url: Url,
) -> Result<T, ConnectorError> {
    let client = http.client_for_url(&url).await?;
    let response = client.get(url).send().await.map_err(|_| {
        ConnectorError::transient(
            ConnectorErrorCode::ProviderUnavailable,
            "provider metadata is unavailable",
        )
    })?;
    if !response.status().is_success() {
        return Err(ConnectorError::transient(
            ConnectorErrorCode::ProviderUnavailable,
            "provider metadata request failed",
        ));
    }
    let bytes = response.bytes().await.map_err(|_| {
        ConnectorError::transient(
            ConnectorErrorCode::ProviderUnavailable,
            "provider metadata could not be read",
        )
    })?;
    if bytes.len() > 1024 * 1024 {
        return Err(ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "provider metadata exceeds one MiB",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        ConnectorError::permanent(
            ConnectorErrorCode::ProtocolError,
            "provider metadata is invalid",
        )
    })
}

fn parse_scope(value: &str) -> Vec<String> {
    value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect()
}

fn reserved_authorization_parameter(name: &str) -> bool {
    matches!(
        name,
        "response_type"
            | "client_id"
            | "redirect_uri"
            | "state"
            | "scope"
            | "nonce"
            | "code_challenge"
            | "code_challenge_method"
    )
}

fn reserved_token_parameter(name: &str) -> bool {
    matches!(
        name,
        "grant_type" | "client_id" | "client_secret" | "code" | "redirect_uri" | "code_verifier"
    )
}

fn access_token_hash(access_token: &str, algorithm: Algorithm) -> Option<String> {
    let digest = match algorithm {
        Algorithm::RS256 | Algorithm::PS256 | Algorithm::ES256 => {
            Sha256::digest(access_token.as_bytes()).to_vec()
        }
        Algorithm::RS384 | Algorithm::PS384 | Algorithm::ES384 => {
            Sha384::digest(access_token.as_bytes()).to_vec()
        }
        Algorithm::RS512 | Algorithm::PS512 | Algorithm::EdDSA => {
            Sha512::digest(access_token.as_bytes()).to_vec()
        }
        _ => return None,
    };
    Some(URL_SAFE_NO_PAD.encode(&digest[..digest.len() / 2]))
}

fn is_pkce_byte(value: u8) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, b'-' | b'.' | b'_' | b'~')
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified())
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_uses_s256_without_padding() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier).unwrap(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn descriptor_rejects_unknown_permissions() {
        let descriptor = ConnectorDescriptor {
            id: "example".into(),
            version: "1".into(),
            display_name: "Example".into(),
            description: String::new(),
            category: "test".into(),
            icon: None,
            auth_methods: vec![AuthMethod::ApiKey],
            permissions: vec![],
            capabilities: vec![ConnectorCapability {
                id: "read".into(),
                label: "Read".into(),
                description: String::new(),
                read_only: true,
                permission_ids: vec!["missing".into()],
                scopes: vec![],
            }],
            allowed_hosts: vec!["example.com".into()],
            public_config_schema: Value::Null,
        };
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn owner_contract_uses_type_discriminator() {
        let owner = OwnerRef {
            owner_type: OwnerType::User,
            user_id: Some(Uuid::nil()),
            team_id: None,
        };
        let value = serde_json::to_value(owner).unwrap();
        assert_eq!(value.get("type").and_then(Value::as_str), Some("user"));
        assert!(value.get("ownerType").is_none());
    }

    #[test]
    fn protocol_parameters_cannot_override_security_bindings() {
        for parameter in [
            "redirect_uri",
            "state",
            "nonce",
            "code_challenge",
            "code_challenge_method",
        ] {
            assert!(reserved_authorization_parameter(parameter));
        }
        for parameter in ["client_secret", "code", "redirect_uri", "code_verifier"] {
            assert!(reserved_token_parameter(parameter));
        }
    }

    #[test]
    fn network_guard_rejects_private_and_special_addresses() {
        for address in ["127.0.0.1", "10.0.0.1", "169.254.1.1", "::1", "fd00::1"] {
            assert!(!is_public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDeliveryPayload {
    pub id: Uuid,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub data: Value,
}

pub fn calculate_webhook_signature(secret: &str, payload_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(payload_bytes);
    let result = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for byte in result {
        hex_str.push_str(&format!("{:02x}", byte));
    }
    hex_str
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookRetryPolicy {
    pub max_attempts: u32,
    pub initial_interval_secs: u64,
    pub max_interval_secs: u64,
    pub backoff_factor: f64,
}

impl Default for WebhookRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_interval_secs: 2,
            max_interval_secs: 300,
            backoff_factor: 2.0,
        }
    }
}

impl WebhookRetryPolicy {
    pub fn next_retry_delay(&self, attempt: u32) -> Duration {
        if attempt >= self.max_attempts {
            return Duration::ZERO;
        }
        let delay_secs = (self.initial_interval_secs as f64
            * self.backoff_factor.powi(attempt as i32 - 1)) as u64;
        Duration::from_secs(delay_secs.min(self.max_interval_secs))
    }
}
