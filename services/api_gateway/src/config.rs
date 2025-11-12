use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
  pub http_addr: String,            // 0.0.0.0:8080
  pub rabbitmq_url: String,         // amqp://user:pass@rabbitmq:5672/%2f
  pub exchange_name: String,        // notifications.direct
  pub redis_url: String,            // redis://redis:6379/0
  pub user_svc_url: String,         // http://user_service:8080
  pub template_svc_url: String,     // http://template_service:8080
  pub idem_ttl_secs: u64,           // 86400
  pub status_ttl_secs: u64          // 86400
}
impl Config { pub fn from_env() -> Self { envy::from_env().expect("invalid env") } }
