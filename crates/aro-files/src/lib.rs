use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use aro_core::{AroError, AroResult};
use async_trait::async_trait;
use aws_sdk_s3::{
    config::{BehaviorVersion, Credentials, Region},
    primitives::ByteStream,
    Client as S3Client,
};
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

const DEFAULT_MAX_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_ALLOWED_MIME: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/csv",
    "application/json",
    "application/pdf",
    "image/png",
    "image/jpeg",
    "image/webp",
];

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub size_bytes: i64,
    pub sha256: String,
    pub etag: Option<String>,
}

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> AroResult<StoredObject>;
    async fn get(&self, key: &str) -> AroResult<Vec<u8>>;
    async fn delete(&self, key: &str) -> AroResult<()>;
    fn backend(&self) -> &'static str;
    fn bucket(&self) -> Option<&str>;
}

#[derive(Debug, Clone)]
pub struct FileStorageSettings {
    pub enabled: bool,
    pub backend: String,
    pub max_bytes: u64,
    pub allowed_mime: HashSet<String>,
    pub local_root: PathBuf,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
}

impl FileStorageSettings {
    pub fn from_env() -> AroResult<Self> {
        let enabled = env_bool("ARO_FILES_API", true);
        let backend = std::env::var("ARO_FILE_STORAGE_BACKEND")
            .unwrap_or_else(|_| "local".to_string())
            .trim()
            .to_ascii_lowercase();
        if !matches!(backend.as_str(), "local" | "s3") {
            return Err(AroError::Configuration(
                "ARO_FILE_STORAGE_BACKEND must be local or s3".to_string(),
            ));
        }

        let max_bytes = std::env::var("ARO_FILE_MAX_BYTES")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_MAX_BYTES);
        let allowed_mime = std::env::var("ARO_FILE_ALLOWED_MIME")
            .ok()
            .map(|value| parse_allowed_mime(&value))
            .unwrap_or_else(|| {
                DEFAULT_ALLOWED_MIME
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect()
            });
        let local_root = std::env::var("ARO_FILE_LOCAL_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("aro-file-storage"));

        Ok(Self {
            enabled,
            backend,
            max_bytes,
            allowed_mime,
            local_root,
            s3_endpoint: std::env::var("ARO_S3_ENDPOINT").ok(),
            s3_bucket: std::env::var("ARO_S3_BUCKET").ok(),
            s3_region: std::env::var("ARO_S3_REGION").ok(),
        })
    }

    pub fn is_allowed_mime(&self, mime_type: &str) -> bool {
        self.allowed_mime.contains(mime_type)
    }
}

#[derive(Debug, Clone)]
pub struct LocalFileStorage {
    root: Arc<PathBuf>,
}

impl LocalFileStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(root.into()),
        }
    }

    fn resolve_key(&self, key: &str) -> AroResult<PathBuf> {
        if key.trim().is_empty() {
            return Err(AroError::Memory("object key is required".to_string()));
        }
        let relative = Path::new(key);
        if relative.is_absolute() {
            return Err(AroError::Security(
                "object key must be relative".to_string(),
            ));
        }
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
        {
            return Err(AroError::Security(
                "object key contains unsafe path components".to_string(),
            ));
        }
        Ok(self.root.join(relative))
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn put(&self, key: &str, bytes: &[u8]) -> AroResult<StoredObject> {
        let path = self.resolve_key(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(map_io)?;
        }
        fs::write(&path, bytes).await.map_err(map_io)?;
        let sha256 = sha256_hex(bytes);
        Ok(StoredObject {
            size_bytes: bytes.len() as i64,
            sha256: sha256.clone(),
            etag: Some(sha256),
        })
    }

    async fn get(&self, key: &str) -> AroResult<Vec<u8>> {
        fs::read(self.resolve_key(key)?).await.map_err(map_io)
    }

    async fn delete(&self, key: &str) -> AroResult<()> {
        match fs::remove_file(self.resolve_key(key)?).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(map_io(err)),
        }
    }

    fn backend(&self) -> &'static str {
        "local"
    }

    fn bucket(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct S3FileStorage {
    bucket: String,
    client: S3Client,
}

impl S3FileStorage {
    pub fn new(
        endpoint: Option<String>,
        bucket: impl Into<String>,
        region: Option<String>,
    ) -> AroResult<Self> {
        let bucket = bucket.into();
        if bucket.trim().is_empty() {
            return Err(AroError::Configuration(
                "ARO_S3_BUCKET is required when ARO_FILE_STORAGE_BACKEND=s3".to_string(),
            ));
        }
        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .or_else(|_| std::env::var("ARO_S3_ACCESS_KEY_ID"))
            .map_err(|_| {
                AroError::Configuration(
                    "AWS_ACCESS_KEY_ID or ARO_S3_ACCESS_KEY_ID is required for S3 storage"
                        .to_string(),
                )
            })?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .or_else(|_| std::env::var("ARO_S3_SECRET_ACCESS_KEY"))
            .map_err(|_| {
                AroError::Configuration(
                    "AWS_SECRET_ACCESS_KEY or ARO_S3_SECRET_ACCESS_KEY is required for S3 storage"
                        .to_string(),
                )
            })?;
        let credentials = Credentials::new(access_key, secret_key, None, None, "aro-env");
        let region = Region::new(region.unwrap_or_else(|| "us-east-1".to_string()));
        let mut config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(region)
            .credentials_provider(credentials);
        if let Some(endpoint) = endpoint {
            config = config.endpoint_url(endpoint).force_path_style(true);
        }
        Ok(Self {
            bucket,
            client: S3Client::from_conf(config.build()),
        })
    }
}

#[async_trait]
impl FileStorage for S3FileStorage {
    async fn put(&self, key: &str, bytes: &[u8]) -> AroResult<StoredObject> {
        let sha256 = sha256_hex(bytes);
        let response = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map_err(map_s3)?;
        Ok(StoredObject {
            size_bytes: bytes.len() as i64,
            sha256,
            etag: response.e_tag().map(trim_s3_etag),
        })
    }

    async fn get(&self, key: &str) -> AroResult<Vec<u8>> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(map_s3)?;
        let bytes = response.body.collect().await.map_err(map_s3)?.into_bytes();
        Ok(bytes.to_vec())
    }

    async fn delete(&self, key: &str) -> AroResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(map_s3)?;
        Ok(())
    }

    fn backend(&self) -> &'static str {
        "s3"
    }

    fn bucket(&self) -> Option<&str> {
        Some(&self.bucket)
    }
}

pub fn build_storage(settings: &FileStorageSettings) -> AroResult<Arc<dyn FileStorage>> {
    match settings.backend.as_str() {
        "local" => Ok(Arc::new(LocalFileStorage::new(settings.local_root.clone()))),
        "s3" => Ok(Arc::new(S3FileStorage::new(
            settings.s3_endpoint.clone(),
            settings.s3_bucket.clone().unwrap_or_default(),
            settings.s3_region.clone(),
        )?)),
        _ => Err(AroError::Configuration(
            "unsupported file storage backend".to_string(),
        )),
    }
}

pub fn storage_key(organization_id: Uuid, file_id: Uuid) -> String {
    format!("organizations/{organization_id}/files/{file_id}/object")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sniff_mime(bytes: &[u8], declared: &str) -> String {
    if bytes.starts_with(b"%PDF-") {
        return "application/pdf".to_string();
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png".to_string();
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return "image/jpeg".to_string();
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp".to_string();
    }
    let normalized = declared.trim().to_ascii_lowercase();
    if normalized == "application/octet-stream" && looks_textual(bytes) {
        return "text/plain".to_string();
    }
    normalized
}

pub fn mime_matches_declared(declared: &str, sniffed: &str) -> bool {
    let declared = declared.trim().to_ascii_lowercase();
    declared == sniffed
        || (declared.starts_with("text/") && sniffed == "text/plain")
        || (declared == "application/octet-stream" && sniffed == "text/plain")
}

fn parse_allowed_mime(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect()
}

fn looks_textual(bytes: &[u8]) -> bool {
    bytes.iter().take(1024).all(|byte| {
        *byte == b'\n' || *byte == b'\r' || *byte == b'\t' || (0x20..=0x7e).contains(byte)
    })
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn map_io(err: std::io::Error) -> AroError {
    AroError::Memory(err.to_string())
}

fn map_s3(err: impl std::fmt::Display) -> AroError {
    AroError::Memory(err.to_string())
}

fn trim_s3_etag(value: &str) -> String {
    value.trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_storage_round_trips_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalFileStorage::new(dir.path());
        let key = storage_key(Uuid::new_v4(), Uuid::new_v4());
        let body = b"hello scalable files";

        let stored = storage.put(&key, body).await.unwrap();
        assert_eq!(stored.size_bytes, body.len() as i64);
        assert_eq!(storage.get(&key).await.unwrap(), body);

        storage.delete(&key).await.unwrap();
        assert!(storage.get(&key).await.is_err());
    }

    #[test]
    fn rejects_unsafe_local_keys() {
        let storage = LocalFileStorage::new(std::env::temp_dir());
        assert!(storage.resolve_key("../escape").is_err());
        assert!(storage.resolve_key("/absolute").is_err());
    }

    #[test]
    fn sniffs_common_mime_types() {
        assert_eq!(
            sniff_mime(b"%PDF-1.7", "application/octet-stream"),
            "application/pdf"
        );
        assert_eq!(
            sniff_mime(b"{\"ok\":true}", "application/json"),
            "application/json"
        );
    }
}
