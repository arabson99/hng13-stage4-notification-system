use anyhow::Result;
use lapin::{Connection, ConnectionProperties, Channel, options::ExchangeDeclareOptions, ExchangeKind, types::FieldTable, BasicProperties};
use serde::Serialize;

#[derive(Clone)]
pub struct Publisher {
  channel: Channel,
  exchange: String,
}
impl Publisher {
  pub async fn connect(amqp: &str, exchange: &str) -> Result<Self> {
    let conn = Connection::connect(amqp, ConnectionProperties::default()).await?;
    let ch = conn.create_channel().await?;
    ch.exchange_declare(exchange, ExchangeKind::Direct, ExchangeDeclareOptions{durable:true, ..Default::default()}, FieldTable::default()).await?;
    Ok(Self { channel: ch, exchange: exchange.into() })
  }
  pub async fn publish<T: Serialize>(&self, routing_key: &str, body: &T) -> Result<()> {
    let payload = serde_json::to_vec(body)?;
    self.channel
      .basic_publish(
        &self.exchange, routing_key,
        lapin::options::BasicPublishOptions::default(),
        &payload,
        BasicProperties::default().with_content_type("application/json".into())
      ).await?.await?;
    Ok(())
  }
}
