use anyhow::Result;
use deadpool_redis::{Config, Pool, Runtime};
use deadpool_redis::redis::{self, AsyncCommands}; // use redis re-exported by deadpool

#[derive(Clone)]
pub struct StatusStore {
  pub pool: Pool,
  pub idem_ttl: u64,
  pub status_ttl: u64,
}

impl StatusStore {
  pub fn new(redis_url: &str, idem_ttl: u64, status_ttl: u64) -> Self {
    let cfg = Config::from_url(redis_url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    Self { pool, idem_ttl, status_ttl }
  }

  pub async fn reserve_idem(&self, key: &str) -> Result<bool> {
    let mut conn = self.pool.get().await?;
    let created: bool = redis::cmd("SETNX")
      .arg(format!("idem:{key}"))
      .arg("1")
      .query_async(&mut *conn)
      .await?;
    if created {
      let _: () = conn.expire(format!("idem:{key}"), self.idem_ttl as i64).await?;
    }
    Ok(created)
  }

  pub async fn set_status(&self, id: &str, state: &str) -> Result<()> {
    let mut conn = self.pool.get().await?;
    let k = format!("notify:status:{id}");
    let now = chrono::Utc::now().to_rfc3339();
    let _: () = redis::pipe()
      .cmd("HSET").arg(&k).arg("state").arg(state).ignore()
      .cmd("HSET").arg(&k).arg("updated_at").arg(&now).ignore()
      .cmd("EXPIRE").arg(&k).arg(self.status_ttl as i64).ignore()
      .query_async(&mut *conn)
      .await?;
    Ok(())
  }
}
