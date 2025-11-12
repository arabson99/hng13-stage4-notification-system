use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType { Email, Push }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UserData {
    pub name: String,
    pub link: String,
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateNotificationRequest {
    pub notification_type: NotificationType,
    pub user_id: Uuid,
    pub template_code: String,
    pub variables: UserData,
    pub request_id: String,
    pub priority: i32,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus { Delivered, Pending, Failed }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateStatusRequest {
    pub notification_id: String,
    pub status: NotificationStatus,
    /// RFC3339 string from workers; gateway will fill if missing
    pub timestamp: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Envelope<T: serde::Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub message: String,
    pub meta: Option<serde_json::Value>,
}

impl<T: serde::Serialize> Envelope<T> {
    pub fn ok(message: &str, data: T) -> Self {
        Self { success: true, data: Some(data), error: None, message: message.into(), meta: None }
    }
    pub fn err(message: &str, err: &str) -> Self {
        Self { success: false, data: None, error: Some(err.into()), message: message.into(), meta: None }
    }
}
