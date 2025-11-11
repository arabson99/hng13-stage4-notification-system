use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use dotenvy::dotenv;
use lapin::{
    options::{
        BasicQosOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
    Channel, Connection, ConnectionProperties, ExchangeKind,
};
use std::{
    env,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;

mod handlers;   // create_notification, create_user, status callbacks, etc.
mod status;     // StatusStore (Redis for idempotency & status)
mod middleware; // CorrelationId
mod models;     // request/response models

use status::StatusStore;
use middleware::CorrelationId;

// ---------- App (shared) state ----------
pub struct AppState {
    pub amqp_channel: Channel,
    pub exchange_name: String,
    pub user_svc_url: String,
    pub template_svc_url: String,
    pub status_store: StatusStore,
    pub amqp_ready: Arc<AtomicBool>,
}

// ---------- Health & readiness ----------
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "ok",
        "data": null,
        "meta": null
    }))
}

async fn ready(state: web::Data<AppState>) -> impl Responder {
    if state.amqp_ready.load(Ordering::SeqCst) {
        HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": "ready",
            "data": null,
            "meta": null
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "success": false,
            "message": "amqp_not_ready",
            "error": "rabbitmq not connected",
            "meta": null
        }))
    }
}

// ---------- AMQP bootstrap helpers ----------
async fn connect_with_retry(amqp_url: &str, max_secs: u64) -> Connection {
    let mut delay = 1u64;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(max_secs);

    loop {
        match Connection::connect(amqp_url, ConnectionProperties::default()).await {
            Ok(conn) => return conn,
            Err(e) if tokio::time::Instant::now() < deadline => {
                eprintln!("AMQP connect failed: {e}. retrying in {delay}s …");
                sleep(Duration::from_secs(delay)).await;
                delay = (delay * 2).min(10);
            }
            Err(e) => panic!("AMQP connect failed and gave up: {e}"),
        }
    }
}

async fn declare_topology(channel: &Channel, exchange: &str) -> Result<()> {
    // exchange
    channel
        .exchange_declare(
            exchange,
            ExchangeKind::Direct,
            ExchangeDeclareOptions {
                passive: false,
                durable: true,
                auto_delete: false,
                internal: false,
                nowait: false,
            },
            FieldTable::default(),
        )
        .await?;

    // queues
    let email_q = "email.queue";
    let push_q = "push.queue";
    let failed_q = "failed.queue";

    channel
        .queue_declare(
            email_q,
            QueueDeclareOptions {
                passive: false,
                durable: true,
                auto_delete: false,
                exclusive: false,
                nowait: false,
            },
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            push_q,
            QueueDeclareOptions {
                passive: false,
                durable: true,
                auto_delete: false,
                exclusive: false,
                nowait: false,
            },
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            failed_q,
            QueueDeclareOptions {
                passive: false,
                durable: true,
                auto_delete: false,
                exclusive: false,
                nowait: false,
            },
            FieldTable::default(),
        )
        .await?;

    // bindings
    channel
        .queue_bind(
            email_q,
            exchange,
            "email",
            QueueBindOptions { nowait: false },
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            push_q,
            exchange,
            "push",
            QueueBindOptions { nowait: false },
            FieldTable::default(),
        )
        .await?;

    // QoS for fairness if we later consume here (gateway usually publishes only)
    channel.basic_qos(0, BasicQosOptions { global: true }).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    // ---- env (with safe defaults for local dev) ----
    let http_addr = env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let amqp_url = env::var("RABBITMQ_URL")
        .unwrap_or_else(|_| "amqp://user:pass@rabbitmq:5672/%2f".to_string());
    let exchange_name =
        env::var("EXCHANGE_NAME").unwrap_or_else(|_| "notifications.direct".to_string());

    let user_svc_url =
        env::var("USER_SVC_URL").unwrap_or_else(|_| "http://user_service:8080".to_string());
    let template_svc_url =
        env::var("TEMPLATE_SVC_URL").unwrap_or_else(|_| "http://template_service:8080".to_string());

    let redis_url =
        env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379/0".to_string());

    let idem_ttl_secs: u64 = env::var("IDEM_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(86_400);

    let status_ttl_secs: u64 = env::var("STATUS_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(86_400);

    // ---- connect to RabbitMQ with retry & declare topology ----
    let amqp_ready = Arc::new(AtomicBool::new(false));
    let conn = connect_with_retry(&amqp_url, 60).await;
    let channel = conn.create_channel().await?;
    declare_topology(&channel, &exchange_name).await?;
    amqp_ready.store(true, Ordering::SeqCst);

    // ---- Redis status store (idempotency + status) ----
    let status_store = StatusStore::new(&redis_url, idem_ttl_secs, status_ttl_secs);

    // ---- app state ----
    let state = web::Data::new(AppState {
        amqp_channel: channel,
        exchange_name,
        user_svc_url,
        template_svc_url,
        status_store,
        amqp_ready: amqp_ready.clone(),
    });

    // ---- HTTP server ----
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(CorrelationId) // x-correlation-id
            .app_data(state.clone())
            .route("/health", web::get().to(health))
            .route("/ready", web::get().to(ready))
            .service(
                web::scope("/api/v1")
                    // notifications
                    .route("/notifications/", web::post().to(handlers::create_notification))
                    // user pass-throughs (gateway validates/forwards)
                    .route("/users/", web::post().to(handlers::create_user))
                    // worker status callbacks (email/push services)
                    .route("/email/status/", web::post().to(handlers::update_status_email))
                    .route("/push/status/", web::post().to(handlers::update_status_push)),
            )
    })
    .bind(http_addr)?
    .workers(num_cpus::get().max(2)) // simple tuning; horizontally scale via replicas
    .run()
    .await?;

    Ok(())
}
