use actix_web::{web, HttpResponse, http::StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{
    CreateNotificationRequest,
    UpdateStatusRequest,
    NotificationStatus,
    NotificationType,
    Envelope,
};
use crate::status::StatusStore;

use lapin::{
    options::BasicPublishOptions,
    protocol::basic::AMQPProperties,
};

#[derive(Clone)]
pub struct AppState {
    pub amqp_channel: lapin::Channel,
    pub exchange_name: String,
    pub user_svc_url: String,        // optional proxy
    pub template_svc_url: String,    // workers use it; kept for completeness
    pub status_store: StatusStore,
    pub amqp_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

pub async fn ready(state: web::Data<AppState>) -> HttpResponse {
    let _ = state.status_store.set_status("ready_probe", "ok").await;
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

// Optional helper for teams to create users via gateway during testing
pub async fn create_user(state: web::Data<AppState>, body: web::Json<Value>) -> HttpResponse {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/users/", state.user_svc_url);

    match client.post(&url).json(&*body).send().await {
        Ok(resp) => {
            let reqwest_status = resp.status();
            let actix_status =
                StatusCode::from_u16(reqwest_status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.json::<Value>().await {
                Ok(v) => HttpResponse::build(actix_status).json(v),
                Err(e) => HttpResponse::BadGateway().json(Envelope::<Value>::err(
                    "user_service_bad_json",
                    &format!("failed to parse user service json: {e}"),
                )),
            }
        }
        Err(e) => HttpResponse::BadGateway().json(Envelope::<Value>::err(
            "user_service_unreachable",
            &format!("failed to reach user service: {e}"),
        )),
    }
}

// POST /api/v1/notifications/
pub async fn create_notification(
    state: web::Data<AppState>,
    body: web::Json<CreateNotificationRequest>,
) -> HttpResponse {
    let req = body.into_inner();

    // Idempotency guard
    match state.status_store.reserve_idem(&req.request_id).await {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Accepted().json(
                Envelope::<Value>::ok("duplicate_request", json!({ "notification_id": req.request_id }))
            )
        }
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json(Envelope::<Value>::err("idempotency_error", "redis_error"))
        }
    }

    // Decide routing key
    let routing_key = match req.notification_type {
        NotificationType::Email => "email",
        NotificationType::Push  => "push",
    };

    // Lean message: workers fetch user & template themselves
    let correlation_id = Uuid::new_v4().to_string();
    let msg = json!({
        "request_id": req.request_id,
        "correlation_id": correlation_id,
        "notification_type": routing_key,
        "user_id": req.user_id,
        "template_code": req.template_code,
        "variables": req.variables,
        "priority": req.priority,
        "metadata": req.metadata,
        "attempt": 0,
        "max_attempts": 3,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });

    // Publish
    let payload = match serde_json::to_vec(&msg) {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError().json(Envelope::<Value>::err(
                "serialize_error",
                &format!("failed to serialize publish payload: {e}"),
            ))
        }
    };

    let publish_res = state
        .amqp_channel
        .basic_publish(
            &state.exchange_name,
            routing_key,
            BasicPublishOptions { mandatory: true, immediate: false },
            &payload,
            AMQPProperties::default()
                .with_correlation_id(correlation_id.clone().into())
                .with_message_id(Uuid::new_v4().to_string().into())
                .with_content_type("application/json".into()),
        )
        .await;

    if publish_res.is_err() {
        let _ = state
            .status_store
            .set_status(msg["request_id"].as_str().unwrap_or_default(), "failed")
            .await;
        return HttpResponse::BadGateway()
            .json(Envelope::<Value>::err("queue_publish_failed", "rabbitmq_error"));
    }

    let _ = state
        .status_store
        .set_status(msg["request_id"].as_str().unwrap_or_default(), "pending")
        .await;

    HttpResponse::Accepted()
        .json(Envelope::<Value>::ok("queued", json!({ "notification_id": msg["request_id"] })))
}

// Workers post back here:

pub async fn update_status_email(
    state: web::Data<AppState>,
    body: web::Json<UpdateStatusRequest>,
) -> HttpResponse {
    update_status_impl("email", state, body).await
}
pub async fn update_status_push(
    state: web::Data<AppState>,
    body: web::Json<UpdateStatusRequest>,
) -> HttpResponse {
    update_status_impl("push", state, body).await
}

async fn update_status_impl(
    channel: &str,
    state: web::Data<AppState>,
    body: web::Json<UpdateStatusRequest>,
) -> HttpResponse {
    let mut req = body.into_inner();

    if req.timestamp.is_none() {
        req.timestamp = Some(chrono::Utc::now().to_rfc3339());
    }

    let state_str = match req.status {
        NotificationStatus::Delivered => "delivered",
        NotificationStatus::Pending   => "pending",
        NotificationStatus::Failed    => "failed",
    };

    if let Err(_) = state.status_store.set_status(&req.notification_id, state_str).await {
        return HttpResponse::InternalServerError()
            .json(Envelope::<Value>::err("status_update_failed", "redis_error"));
    }

    HttpResponse::Ok().json(Envelope::<Value>::ok(
        "status_updated",
        json!({
            "notification_id": req.notification_id,
            "status": state_str,
            "channel": channel,
            "timestamp": req.timestamp,
            "error": req.error
        }),
    ))
}
