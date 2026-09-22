use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
    AgentCompletion,
    Routine,
    System,
    Security,
}

impl Default for NotificationKind {
    fn default() -> Self {
        Self::Info
    }
}

impl NotificationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::AgentCompletion => "agent-completion",
            Self::Routine => "routine",
            Self::System => "system",
            Self::Security => "security",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Default for NotificationPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl NotificationPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationStatus {
    Unread,
    Read,
    Archived,
}

impl Default for NotificationStatus {
    fn default() -> Self {
        Self::Unread
    }
}

impl NotificationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Read => "read",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationSource {
    Agent,
    Routine,
    System,
    Cloud,
}

impl Default for NotificationSource {
    fn default() -> Self {
        Self::System
    }
}

impl NotificationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Routine => "routine",
            Self::System => "system",
            Self::Cloud => "cloud",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: String,
    pub organization_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub kind: NotificationKind,
    #[serde(default)]
    pub priority: NotificationPriority,
    #[serde(default)]
    pub status: NotificationStatus,
    #[serde(default)]
    pub source: NotificationSource,
    pub action_url: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

impl NotificationItem {
    pub fn new(
        title: impl Into<String>,
        body: impl Into<String>,
        kind: NotificationKind,
        source: NotificationSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            organization_id: None,
            user_id: None,
            title: title.into(),
            body: body.into(),
            kind,
            priority: NotificationPriority::Normal,
            status: NotificationStatus::Unread,
            source,
            action_url: None,
            metadata: None,
            created_at: Utc::now(),
            read_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationFilter {
    pub status: Option<NotificationStatus>,
    pub kind: Option<NotificationKind>,
    pub source: Option<NotificationSource>,
    pub search: Option<String>,
    pub organization_id: Option<Uuid>,
    #[serde(default)]
    pub personal_only: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    pub desktop_notifications_enabled: bool,
    pub sound_enabled: bool,
    pub agent_completion_notifications: bool,
    pub routine_notifications: bool,
    pub email_notifications_enabled: bool,
    pub email_on_agent_completion: bool,
    pub email_on_routine_summary: bool,
    pub email_recipient: Option<String>,
    pub email_provider: String,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_user: Option<String>,
    #[serde(skip)]
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub smtp_tls_mode: Option<String>,
    #[serde(skip)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_configured: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop_notifications_enabled: true,
            sound_enabled: true,
            agent_completion_notifications: true,
            routine_notifications: true,
            email_notifications_enabled: false,
            email_on_agent_completion: false,
            email_on_routine_summary: false,
            email_recipient: None,
            email_provider: "smtp".into(),
            smtp_host: None,
            smtp_port: Some(587),
            smtp_user: None,
            smtp_password: None,
            smtp_from: Some("noreply@aro-ai.com".into()),
            smtp_tls_mode: Some("starttls".into()),
            api_key: None,
            auth_configured: false,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_item_initialization_defaults() {
        let item = NotificationItem::new(
            "Agent Finished",
            "Data pipeline completed successfully",
            NotificationKind::AgentCompletion,
            NotificationSource::Agent,
        );

        assert_eq!(item.status, NotificationStatus::Unread);
        assert_eq!(item.priority, NotificationPriority::Normal);
        assert_eq!(item.kind, NotificationKind::AgentCompletion);
        assert_eq!(item.source, NotificationSource::Agent);
        assert!(item.read_at.is_none());
        assert!(!item.id.is_empty());
    }

    #[test]
    fn notification_settings_serializes_roundtrip() {
        let settings = NotificationSettings::default();
        let serialized = serde_json::to_string(&settings).expect("serialize settings");
        let deserialized: NotificationSettings =
            serde_json::from_str(&serialized).expect("deserialize settings");
        assert_eq!(settings, deserialized);
        assert!(deserialized.desktop_notifications_enabled);
        assert_eq!(deserialized.email_provider, "smtp");
    }
}
