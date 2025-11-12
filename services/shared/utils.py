import redis
import json
from typing import Optional, Any
from functools import wraps
import time
import logging

logger = logging.getLogger(__name__)

class RedisCache:
    def __init__(self, redis_url: str):
        self.redis_client = redis.from_url(redis_url, decode_responses=True)
    
    def get(self, key: str) -> Optional[Any]:
        try:
            value = self.redis_client.get(key)
            return json.loads(value) if value else None
        except Exception as e:
            logger.error(f"Redis get error: {e}")
            return None
    
    def set(self, key: str, value: Any, ttl: int = 300):
        try:
            self.redis_client.setex(key, ttl, json.dumps(value))
        except Exception as e:
            logger.error(f"Redis set error: {e}")
    
    def delete(self, key: str):
        try:
            self.redis_client.delete(key)
        except Exception as e:
            logger.error(f"Redis delete error: {e}")

    def ping(self) -> bool:
        """Check Redis connection health."""
        try:
            return self.redis_client.ping()
        except Exception as e:
            logger.error(f"Redis ping error: {e}")
            return False

