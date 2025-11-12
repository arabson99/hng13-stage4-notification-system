import pika
import json
import logging
import asyncio
import httpx
import os
import sys
from datetime import datetime
from sqlalchemy import create_engine, Column, String, DateTime, Text
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import sessionmaker
from sqlalchemy.dialects.postgresql import UUID
import uuid

from smtp_handler import SMTPHandler

from shared.utils import RedisCache

# Initialize Redis
REDIS_URL = os.getenv("REDIS_URL", "redis://localhost:6379")
cache = RedisCache(REDIS_URL)


logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Database setup
DATABASE_URL = os.getenv("DATABASE_URL")
engine = create_engine(DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
Base = declarative_base()

class NotificationLog(Base):
    __tablename__ = "email_notifications"
    
    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4)
    notification_id = Column(String, unique=True, nullable=False, index=True)
    user_id = Column(String, nullable=False)
    email = Column(String, nullable=False)
    subject = Column(String)
    body = Column(Text)
    status = Column(String, default="pending")
    error_message = Column(Text)
    retry_count = Column(String, default="0")
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

Base.metadata.create_all(bind=engine)

# Configuration
RABBITMQ_URL = os.getenv("RABBITMQ_URL")
TEMPLATE_SERVICE_URL = os.getenv("TEMPLATE_SERVICE_URL")
USER_SERVICE_URL = os.getenv("USER_SERVICE_URL", "http://user-service:8001")
MAX_RETRIES = 3

smtp_handler = SMTPHandler()

async def fetch_user_email(user_id: str) -> str:
    """Fetch user email from user service"""
    async with httpx.AsyncClient() as client:
        response = await client.get(f"{USER_SERVICE_URL}/api/v1/users/{user_id}")
        if response.status_code == 200:
            data = response.json()
            return data["data"]["email"]
    raise Exception("User not found")

async def render_template(template_code: str, variables: dict):
    """Render template with variables"""
    async with httpx.AsyncClient() as client:
        response = await client.post(
            f"{TEMPLATE_SERVICE_URL}/api/v1/templates/{template_code}/render",
            json=variables
        )
        if response.status_code == 200:
            return response.json()["data"]
    raise Exception("Template rendering failed")

async def process_email(message_data: dict, retry_count: int = 0):
    """Process email notification"""
    notification_id = message_data["notification_id"]
    user_id = message_data["user_id"]
    
    db = SessionLocal()
    
    try:
        # Check if already processed
        existing = db.query(NotificationLog).filter(
            NotificationLog.notification_id == notification_id
        ).first()
        
        if existing and existing.status == "delivered":
            logger.info(f"Email {notification_id} already delivered")
            return True
        
        # Set status: processing
        cache.set(
            f"notification_status:{notification_id}",
            {"status": "processing", "updated_at": datetime.utcnow().isoformat()},
            ttl=86400
        )

        # Fetch user email
        user_email = await fetch_user_email(user_id)
        
        # Render template
        rendered = await render_template(
            message_data["template_code"],
            message_data["variables"]
        )
        
        subject = rendered.get("subject", "Notification")
        body = rendered["body"]
        
        # Send email
        await smtp_handler.send_email(user_email, subject, body)
        
        # Log success
        if existing:
            existing.status = "delivered"
            existing.updated_at = datetime.utcnow()
        else:
            log = NotificationLog(
                notification_id=notification_id,
                user_id=user_id,
                email=user_email,
                subject=subject,
                body=body,
                status="delivered"
            )
            db.add(log)
        
        db.commit()

        cache.set(
            f"notification_status:{notification_id}",
            {"status": "delivered", "delivered_at": datetime.utcnow().isoformat()},
            ttl=86400
        )

        logger.info(f"Email {notification_id} delivered to {user_email}")
        return True
        
    except Exception as e:
        logger.error(f"Email processing failed: {e}")

        # Update cache for failure
        cache.set(
            f"notification_status:{notification_id}",
            {"status": "failed", "error": str(e), "failed_at": datetime.utcnow().isoformat()},
            ttl=86400
        )
        
        # Log failure
        if retry_count < MAX_RETRIES:
            # Will be retried
            if existing:
                existing.retry_count = str(retry_count + 1)
                existing.error_message = str(e)
                db.commit()
            return False
        else:
            # Permanent failure
            if existing:
                existing.status = "failed"
                existing.error_message = str(e)
            else:
                log = NotificationLog(
                    notification_id=notification_id,
                    user_id=user_id,
                    email="unknown",
                    status="failed",
                    error_message=str(e),
                    retry_count=str(retry_count)
                )
                db.add(log)
            db.commit()
            raise e
    finally:
        db.close()

def callback(ch, method, properties, body):
    """RabbitMQ message callback"""
    try:
        message_data = json.loads(body)
        logger.info(f"Processing email notification: {message_data['notification_id']}")
        
        # Process with retry logic
        retry_count = int(message_data.get("retry_count", 0))
        
        success = asyncio.run(process_email(message_data, retry_count))
        
        if success:
            ch.basic_ack(delivery_tag=method.delivery_tag)
        else:
            # Retry with exponential backoff
            if retry_count < MAX_RETRIES:
                message_data["retry_count"] = retry_count + 1
                
                # Re-queue with delay (exponential backoff)
                delay = (2 ** retry_count) * 1000  # milliseconds
                
                ch.basic_publish(
                    exchange='notifications.direct',
                    routing_key='email',
                    body=json.dumps(message_data),
                    properties=pika.BasicProperties(
                        delivery_mode=2,
                        headers={'x-delay': delay}
                    )
                )
                ch.basic_ack(delivery_tag=method.delivery_tag)
                logger.info(f"Email {message_data['notification_id']} requeued for retry {retry_count + 1}")
            else:
                # Move to dead letter queue
                ch.basic_publish(
                    exchange='notifications.direct',
                    routing_key='failed',
                    body=body
                )
                ch.basic_ack(delivery_tag=method.delivery_tag)
                logger.error(f"Email {message_data['notification_id']} moved to DLQ after {MAX_RETRIES} retries")
                
    except Exception as e:
        logger.error(f"Callback error: {e}")
        ch.basic_nack(delivery_tag=method.delivery_tag, requeue=False)

def main():
    """Start email consumer"""
    logger.info("Starting email service consumer...")
    
    connection = pika.BlockingConnection(pika.URLParameters(RABBITMQ_URL))
    channel = connection.channel()
    
    # Set QoS to process one message at a time
    channel.basic_qos(prefetch_count=1)
    
    # Start consuming
    channel.basic_consume(
        queue='email.queue',
        on_message_callback=callback
    )
    
    logger.info("Email consumer started. Waiting for messages...")
    channel.start_consuming()

if __name__ == "__main__":
    main()