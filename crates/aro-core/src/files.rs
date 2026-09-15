use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FileStatus {
    Pending,
    Available,
    Quarantined,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FileScanStatus {
    Pending,
    Clean,
    Blocked,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentMode {
    CloudObject,
    LocalReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileObject {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub status: FileStatus,
    pub scan_status: FileScanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadSession {
    pub id: Uuid,
    pub file_id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub expected_name: String,
    pub expected_mime_type: String,
    pub expected_size_bytes: i64,
    pub expected_sha256: Option<String>,
    pub status: FileStatus,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRef {
    pub file_id: Option<Uuid>,
    pub mode: AttachmentMode,
    pub display_name: String,
    pub size_bytes: i64,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageAttachment {
    pub file_id: Option<Uuid>,
    pub mode: AttachmentMode,
    pub display_name: String,
    pub size_bytes: i64,
    pub mime_type: String,
}

impl From<AttachmentRef> for MessageAttachment {
    fn from(value: AttachmentRef) -> Self {
        Self {
            file_id: value.file_id,
            mode: value.mode,
            display_name: value.display_name,
            size_bytes: value.size_bytes,
            mime_type: value.mime_type,
        }
    }
}

impl From<&AttachmentRef> for MessageAttachment {
    fn from(value: &AttachmentRef) -> Self {
        Self {
            file_id: value.file_id,
            mode: value.mode.clone(),
            display_name: value.display_name.clone(),
            size_bytes: value.size_bytes,
            mime_type: value.mime_type.clone(),
        }
    }
}
