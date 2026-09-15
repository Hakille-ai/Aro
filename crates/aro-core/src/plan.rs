use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn default_task_status() -> Option<String> {
    Some("pending".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStep {
    pub id: String,
    pub text: String,
    pub completed: bool,
    #[serde(
        default = "default_task_status",
        skip_serializing_if = "Option::is_none"
    )]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TaskStep {
    pub fn new(id: impl Into<String>, text: impl Into<String>, completed: bool) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            completed,
            status: default_task_status(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub tasks: Vec<TaskStep>,
    pub status: String, // "active", "completed", "archived"
    pub created_at: String,
    pub updated_at: String,
}
