use anyhow::Result;
use deadpool_redis::{Config, Pool, Runtime};
use deadpool_redis::redis;                        
use deadpool_redis::redis::AsyncCommands; 

#[derive(Clone)]
pub struct StatusStore {
    pool: Pool,
    idem_ttl: usize,
    status_ttl: usize,
}

impl StatusStore {
    pub fn new(redis_url: &str, idem_ttl_secs: u64, status_ttl_secs: u64) -> Self {
        let cfg = Config::from_url(redis_url.to_string());
        let pool = cfg.create_pool(Some(Runtime::Tokio1)).expect("redis pool");
        Self {
            pool,
            idem_ttl: idem_ttl_secs as usize,
            status_ttl: status_ttl_secs as usize,
        }
    }

    /// Reserve idempotency key; returns true if we reserved, false if duplicate.
    pub async fn reserve_idem(&self, req_id: &str) -> Result<bool> {
        let mut conn = self.pool.get().await?;
        // SETNX + EX
        let key = format!("idem:{}", req_id);
        let created: bool = redis::cmd("SET")
            .arg(&key).arg("1")
            .arg("NX")
            .arg("EX").arg(self.idem_ttl)
            .query_async(&mut *conn).await
            .unwrap_or(false);
        Ok(created)
    }

    pub async fn set_status(&self, notification_id: &str, state: &str) -> Result<()> {
        let mut conn = self.pool.get().await?;
        let key = format!("status:{}", notification_id);
        let _: () = redis::pipe()
            .cmd("SET").arg(&key).arg(state).ignore()
            .cmd("EXPIRE").arg(&key).arg(self.status_ttl).ignore()
            .query_async(&mut *conn).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_status(&self, notification_id: &str) -> Result<Option<String>> {
        let mut conn = self.pool.get().await?;
        let key = format!("status:{}", notification_id);
        let v: Option<String> = conn.get(key).await?;
        Ok(v)
    }
}
