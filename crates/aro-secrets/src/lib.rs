//! Application-layer envelope encryption for integration credentials.

use std::{fmt, sync::Arc, time::Duration};

use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_kms::{primitives::Blob, Client as KmsClient};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{rngs::OsRng, RngCore};
use reqwest::{header::HeaderValue, Client, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

const AES_256_GCM: &str = "AES-256-GCM";
const NONCE_BYTES: usize = 12;
const KEY_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret provider is misconfigured")]
    Misconfigured,
    #[error("secret provider is unavailable")]
    ProviderUnavailable,
    #[error("credential encryption failed")]
    EncryptionFailed,
    #[error("credential decryption failed")]
    DecryptionFailed,
    #[error("credential envelope is invalid")]
    InvalidEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialAad {
    pub organization_id: Uuid,
    pub installation_id: Uuid,
    pub provider_id: String,
    pub credential_version: i32,
    pub environment: String,
}

impl CredentialAad {
    fn encoded(&self) -> Result<Vec<u8>, SecretError> {
        serde_json::to_vec(self).map_err(|_| SecretError::InvalidEnvelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationAttemptAad {
    pub organization_id: Uuid,
    pub attempt_id: Uuid,
    pub provider_id: String,
    pub environment: String,
}

impl AuthorizationAttemptAad {
    fn encoded(&self) -> Result<Vec<u8>, SecretError> {
        serde_json::to_vec(self).map_err(|_| SecretError::InvalidEnvelope)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialEnvelope {
    pub algorithm: String,
    pub key_id: String,
    pub key_version: String,
    pub nonce: Vec<u8>,
    pub encrypted_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for CredentialEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialEnvelope")
            .field("algorithm", &self.algorithm)
            .field("key_id", &self.key_id)
            .field("key_version", &self.key_version)
            .field("nonce_bytes", &self.nonce.len())
            .field("encrypted_dek_bytes", &self.encrypted_dek.len())
            .field("ciphertext", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct WrappedKey {
    pub key_id: String,
    pub key_version: String,
    pub ciphertext: Vec<u8>,
}

#[async_trait]
pub trait KeyManagementProvider: Send + Sync {
    async fn wrap_key(&self, plaintext_dek: &[u8]) -> Result<WrappedKey, SecretError>;
    async fn unwrap_key(&self, wrapped: &WrappedKey) -> Result<Zeroizing<Vec<u8>>, SecretError>;
    fn production_ready(&self) -> bool;
    fn provider_name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct EnvelopeCrypto {
    key_provider: Arc<dyn KeyManagementProvider>,
}

impl EnvelopeCrypto {
    pub fn new(key_provider: Arc<dyn KeyManagementProvider>) -> Self {
        Self { key_provider }
    }

    pub fn production_ready(&self) -> bool {
        self.key_provider.production_ready()
    }

    pub async fn encrypt(
        &self,
        plaintext: &[u8],
        aad: &CredentialAad,
    ) -> Result<CredentialEnvelope, SecretError> {
        self.encrypt_with_aad(plaintext, aad.encoded()?).await
    }

    pub async fn encrypt_authorization_attempt(
        &self,
        plaintext: &[u8],
        aad: &AuthorizationAttemptAad,
    ) -> Result<CredentialEnvelope, SecretError> {
        self.encrypt_with_aad(plaintext, aad.encoded()?).await
    }

    async fn encrypt_with_aad(
        &self,
        plaintext: &[u8],
        aad: Vec<u8>,
    ) -> Result<CredentialEnvelope, SecretError> {
        let mut dek = Zeroizing::new([0_u8; KEY_BYTES]);
        OsRng.fill_bytes(dek.as_mut());
        let cipher =
            Aes256Gcm::new_from_slice(dek.as_ref()).map_err(|_| SecretError::EncryptionFailed)?;
        let mut nonce = [0_u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| SecretError::EncryptionFailed)?;
        let wrapped = self.key_provider.wrap_key(dek.as_ref()).await?;
        Ok(CredentialEnvelope {
            algorithm: AES_256_GCM.to_string(),
            key_id: wrapped.key_id,
            key_version: wrapped.key_version,
            nonce: nonce.to_vec(),
            encrypted_dek: wrapped.ciphertext,
            ciphertext,
        })
    }

    pub async fn decrypt(
        &self,
        envelope: &CredentialEnvelope,
        aad: &CredentialAad,
    ) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        self.decrypt_with_aad(envelope, aad.encoded()?).await
    }

    pub async fn decrypt_authorization_attempt(
        &self,
        envelope: &CredentialEnvelope,
        aad: &AuthorizationAttemptAad,
    ) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        self.decrypt_with_aad(envelope, aad.encoded()?).await
    }

    async fn decrypt_with_aad(
        &self,
        envelope: &CredentialEnvelope,
        aad: Vec<u8>,
    ) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        if envelope.algorithm != AES_256_GCM || envelope.nonce.len() != NONCE_BYTES {
            return Err(SecretError::InvalidEnvelope);
        }
        let wrapped = WrappedKey {
            key_id: envelope.key_id.clone(),
            key_version: envelope.key_version.clone(),
            ciphertext: envelope.encrypted_dek.clone(),
        };
        let dek = self.key_provider.unwrap_key(&wrapped).await?;
        if dek.len() != KEY_BYTES {
            return Err(SecretError::InvalidEnvelope);
        }
        let cipher =
            Aes256Gcm::new_from_slice(dek.as_ref()).map_err(|_| SecretError::DecryptionFailed)?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&envelope.nonce),
                Payload {
                    msg: &envelope.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| SecretError::DecryptionFailed)?;
        Ok(Zeroizing::new(plaintext))
    }

    pub async fn rewrap(
        &self,
        envelope: &CredentialEnvelope,
    ) -> Result<CredentialEnvelope, SecretError> {
        let wrapped = WrappedKey {
            key_id: envelope.key_id.clone(),
            key_version: envelope.key_version.clone(),
            ciphertext: envelope.encrypted_dek.clone(),
        };
        let dek = self.key_provider.unwrap_key(&wrapped).await?;
        let next = self.key_provider.wrap_key(dek.as_ref()).await?;
        let mut envelope = envelope.clone();
        envelope.key_id = next.key_id;
        envelope.key_version = next.key_version;
        envelope.encrypted_dek = next.ciphertext;
        Ok(envelope)
    }
}

/// Development-only key provider. Production readiness explicitly rejects this provider.
pub struct LocalKeyProvider {
    key_id: String,
    key_version: String,
    master_key: Zeroizing<[u8; KEY_BYTES]>,
}

impl LocalKeyProvider {
    pub fn new(
        key_id: impl Into<String>,
        key_version: impl Into<String>,
        master_key: [u8; KEY_BYTES],
    ) -> Self {
        Self {
            key_id: key_id.into(),
            key_version: key_version.into(),
            master_key: Zeroizing::new(master_key),
        }
    }
}

#[async_trait]
impl KeyManagementProvider for LocalKeyProvider {
    async fn wrap_key(&self, plaintext_dek: &[u8]) -> Result<WrappedKey, SecretError> {
        let cipher = Aes256Gcm::new_from_slice(self.master_key.as_ref())
            .map_err(|_| SecretError::EncryptionFailed)?;
        let mut nonce = [0_u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let mut ciphertext = nonce.to_vec();
        ciphertext.extend(
            cipher
                .encrypt(Nonce::from_slice(&nonce), plaintext_dek)
                .map_err(|_| SecretError::EncryptionFailed)?,
        );
        Ok(WrappedKey {
            key_id: self.key_id.clone(),
            key_version: self.key_version.clone(),
            ciphertext,
        })
    }

    async fn unwrap_key(&self, wrapped: &WrappedKey) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        if wrapped.key_id != self.key_id
            || wrapped.key_version != self.key_version
            || wrapped.ciphertext.len() <= NONCE_BYTES
        {
            return Err(SecretError::InvalidEnvelope);
        }
        let (nonce, ciphertext) = wrapped.ciphertext.split_at(NONCE_BYTES);
        let cipher = Aes256Gcm::new_from_slice(self.master_key.as_ref())
            .map_err(|_| SecretError::DecryptionFailed)?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| SecretError::DecryptionFailed)?;
        Ok(Zeroizing::new(plaintext))
    }

    fn production_ready(&self) -> bool {
        false
    }

    fn provider_name(&self) -> &'static str {
        "local-development"
    }
}

/// Minimal interface implemented by the deployment's official AWS SDK client.
#[async_trait]
pub trait AwsKmsClient: Send + Sync {
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, SecretError>;
    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>, SecretError>;
}

pub struct AwsSdkKmsClient {
    client: KmsClient,
}

impl AwsSdkKmsClient {
    /// Loads the standard AWS credential/region chain, supporting workload identity and instance
    /// roles without placing long-lived AWS credentials in application configuration.
    pub async fn from_environment(endpoint_url: Option<String>) -> Result<Self, SecretError> {
        let shared = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let mut builder = aws_sdk_kms::config::Builder::from(&shared);
        if let Some(endpoint_url) = endpoint_url {
            if !endpoint_url.starts_with("https://") {
                return Err(SecretError::Misconfigured);
            }
            builder = builder.endpoint_url(endpoint_url);
        }
        Ok(Self {
            client: KmsClient::from_conf(builder.build()),
        })
    }
}

#[async_trait]
impl AwsKmsClient for AwsSdkKmsClient {
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, SecretError> {
        let response = self
            .client
            .encrypt()
            .key_id(key_id)
            .plaintext(Blob::new(plaintext))
            .send()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        response
            .ciphertext_blob()
            .map(|blob| blob.as_ref().to_vec())
            .ok_or(SecretError::ProviderUnavailable)
    }

    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>, SecretError> {
        let response = self
            .client
            .decrypt()
            .key_id(key_id)
            .ciphertext_blob(Blob::new(ciphertext))
            .send()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        response
            .plaintext()
            .map(|blob| blob.as_ref().to_vec())
            .ok_or(SecretError::ProviderUnavailable)
    }
}

pub struct AwsKmsProvider<C> {
    client: C,
    key_id: String,
    key_version: String,
}

impl<C> AwsKmsProvider<C> {
    pub fn new(client: C, key_id: impl Into<String>, key_version: impl Into<String>) -> Self {
        Self {
            client,
            key_id: key_id.into(),
            key_version: key_version.into(),
        }
    }
}

#[async_trait]
impl<C: AwsKmsClient> KeyManagementProvider for AwsKmsProvider<C> {
    async fn wrap_key(&self, plaintext_dek: &[u8]) -> Result<WrappedKey, SecretError> {
        Ok(WrappedKey {
            key_id: self.key_id.clone(),
            key_version: self.key_version.clone(),
            ciphertext: self.client.encrypt(&self.key_id, plaintext_dek).await?,
        })
    }

    async fn unwrap_key(&self, wrapped: &WrappedKey) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        Ok(Zeroizing::new(
            self.client
                .decrypt(&wrapped.key_id, &wrapped.ciphertext)
                .await?,
        ))
    }

    fn production_ready(&self) -> bool {
        true
    }
    fn provider_name(&self) -> &'static str {
        "aws-kms"
    }
}

pub struct VaultTransitProvider {
    client: Client,
    base_url: Url,
    mount: String,
    key_name: String,
    token: Zeroizing<String>,
}

impl VaultTransitProvider {
    pub fn new(
        base_url: Url,
        mount: impl Into<String>,
        key_name: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, SecretError> {
        if base_url.scheme() != "https" || base_url.host_str().is_none() {
            return Err(SecretError::Misconfigured);
        }
        let mount = mount.into();
        let key_name = key_name.into();
        if !safe_path_component(&mount) || !safe_path_component(&key_name) {
            return Err(SecretError::Misconfigured);
        }
        let token = token.into();
        HeaderValue::from_str(&token).map_err(|_| SecretError::Misconfigured)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| SecretError::Misconfigured)?;
        Ok(Self {
            client,
            base_url,
            mount,
            key_name,
            token: Zeroizing::new(token),
        })
    }

    fn endpoint(&self, operation: &str) -> Result<Url, SecretError> {
        self.base_url
            .join(&format!(
                "v1/{}/{}/{}",
                self.mount, operation, self.key_name
            ))
            .map_err(|_| SecretError::Misconfigured)
    }
}

#[derive(Deserialize)]
struct VaultResponse<T> {
    data: T,
}
#[derive(Deserialize)]
struct VaultEncryptData {
    ciphertext: String,
}
#[derive(Deserialize)]
struct VaultDecryptData {
    plaintext: String,
}

#[async_trait]
impl KeyManagementProvider for VaultTransitProvider {
    async fn wrap_key(&self, plaintext_dek: &[u8]) -> Result<WrappedKey, SecretError> {
        let response = self
            .client
            .post(self.endpoint("encrypt")?)
            .header("X-Vault-Token", self.token.as_str())
            .json(&serde_json::json!({ "plaintext": STANDARD.encode(plaintext_dek) }))
            .send()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(SecretError::ProviderUnavailable);
        }
        let payload: VaultResponse<VaultEncryptData> = response
            .json()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        Ok(WrappedKey {
            key_id: format!("vault-transit/{}/{}", self.mount, self.key_name),
            key_version: payload
                .data
                .ciphertext
                .split(':')
                .nth(1)
                .unwrap_or("unknown")
                .to_string(),
            ciphertext: payload.data.ciphertext.into_bytes(),
        })
    }

    async fn unwrap_key(&self, wrapped: &WrappedKey) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        let ciphertext =
            std::str::from_utf8(&wrapped.ciphertext).map_err(|_| SecretError::InvalidEnvelope)?;
        let response = self
            .client
            .post(self.endpoint("decrypt")?)
            .header("X-Vault-Token", self.token.as_str())
            .json(&serde_json::json!({ "ciphertext": ciphertext }))
            .send()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(SecretError::ProviderUnavailable);
        }
        let payload: VaultResponse<VaultDecryptData> = response
            .json()
            .await
            .map_err(|_| SecretError::ProviderUnavailable)?;
        let plaintext = STANDARD
            .decode(payload.data.plaintext)
            .map_err(|_| SecretError::InvalidEnvelope)?;
        Ok(Zeroizing::new(plaintext))
    }

    fn production_ready(&self) -> bool {
        true
    }
    fn provider_name(&self) -> &'static str {
        "vault-transit"
    }
}

fn safe_path_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    fn aad() -> CredentialAad {
        CredentialAad {
            organization_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            provider_id: "example".into(),
            credential_version: 1,
            environment: "test".into(),
        }
    }

    #[test]
    fn envelope_debug_is_redacted() {
        let envelope = CredentialEnvelope {
            algorithm: AES_256_GCM.into(),
            key_id: "key".into(),
            key_version: "1".into(),
            nonce: vec![0; NONCE_BYTES],
            encrypted_dek: vec![1, 2, 3],
            ciphertext: b"super-secret".to_vec(),
        };
        let rendered = format!("{envelope:?}");
        assert!(!rendered.contains("super-secret"));
    }

    #[tokio::test]
    async fn aad_prevents_cross_tenant_decryption() {
        let provider = Arc::new(LocalKeyProvider::new("local", "1", [7; KEY_BYTES]));
        let crypto = EnvelopeCrypto::new(provider);
        let source_aad = aad();
        let envelope = crypto.encrypt(b"secret", &source_aad).await.unwrap();
        assert_eq!(
            crypto
                .decrypt(&envelope, &source_aad)
                .await
                .unwrap()
                .as_slice(),
            b"secret"
        );
        let mut other_aad = source_aad.clone();
        other_aad.organization_id = Uuid::new_v4();
        assert!(matches!(
            crypto.decrypt(&envelope, &other_aad).await,
            Err(SecretError::DecryptionFailed)
        ));
    }

    #[tokio::test]
    async fn authorization_attempt_aad_prevents_cross_attempt_decryption() {
        let provider = Arc::new(LocalKeyProvider::new("local", "1", [9; KEY_BYTES]));
        let crypto = EnvelopeCrypto::new(provider);
        let source = AuthorizationAttemptAad {
            organization_id: Uuid::new_v4(),
            attempt_id: Uuid::new_v4(),
            provider_id: "example".into(),
            environment: "test".into(),
        };
        let envelope = crypto
            .encrypt_authorization_attempt(b"pkce-and-nonce", &source)
            .await
            .unwrap();
        let mut other = source.clone();
        other.attempt_id = Uuid::new_v4();
        assert!(matches!(
            crypto
                .decrypt_authorization_attempt(&envelope, &other)
                .await,
            Err(SecretError::DecryptionFailed)
        ));
    }

    #[test]
    fn plaintext_buffers_are_zeroizable() {
        let mut bytes = b"secret".to_vec();
        bytes.zeroize();
        assert!(bytes.is_empty() || bytes.iter().all(|byte| *byte == 0));
    }
}
