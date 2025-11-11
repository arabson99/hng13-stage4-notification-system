use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use url::Url;
use chrono::{DateTime, Utc};

/// Generic envelope that matches the required response format
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Envelope<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub message: String,
    pub meta: Option<PaginationMeta>,
}
impl<T: Serialize> Envelope<T> {
    pub fn ok(message: &str, data: T) -> Self {
        Self { success: true, data: Some(data), error: None, message: message.into(), meta: None }
    }
    pub fn ok_no_data(message: &str) -> Self {
        Self { success: true, data: None, error: None, message: message.into(), meta: None }
    }
    pub fn err(code: &str, message: &str) -> Self {
        Self { success: false, data: None, error: Some(code.into()), message: message.into(), meta: None }
    }
}

/// Pagination meta required by the spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PaginationMeta {
    pub total: i64,
    pub limit: i64,
    pub page: i64,
    pub total_pages: i64,
    pub has_next: bool,
    pub has_previous: bool,
}

/// Notification types per spec
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType { Email, Push }

/// Variables payload (user-facing substitution data)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UserData {
    pub name: String,
    pub link: Url,                 // validates/serialises as an absolute URL
    pub meta: Option<Value>,
}

/// POST /api/v1/notifications/
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

/// Status values per spec
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus { Delivered, Pending, Failed }

/// POST /api/v1/{notification_preference}/status/
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateStatusRequest {
    pub notification_id: String,
    pub status: NotificationStatus,
    pub timestamp: Option<DateTime<Utc>>,  // proper ISO-8601 via chrono
    pub error: Option<String>,
}

/// User preferences used by User Service
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UserPreference {
    pub email: bool,
    pub push: bool,
}

/// POST /api/v1/users/
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,                // validate in user service
    pub push_token: Option<String>,
    pub preferences: UserPreference,
    pub password: String,
}
