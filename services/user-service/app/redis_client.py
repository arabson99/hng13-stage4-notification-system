import redis
from app.config import settings
from typing import Optional
import json

redis_client: Optional[redis.Redis] = None


def get_redis():
    """Get Redis client instance"""
    global redis_client
    if redis_client is None:
        redis_client = redis.Redis(
            host=settings.redis_host,
            port=settings.redis_port,
            db=settings.redis_db,
            password=settings.redis_password,
            decode_responses=True
        )
    return redis_client


def cache_user_preferences(user_id: str, preferences: dict, ttl: int = 3600):
    """Cache user notification preferences"""
    redis = get_redis()
    key = f"user_preferences:{user_id}"
    redis.setex(key, ttl, json.dumps(preferences))


def get_cached_preferences(user_id: str) -> Optional[dict]:
    """Get cached user notification preferences"""
    redis = get_redis()
    key = f"user_preferences:{user_id}"
    cached = redis.get(key)
    if cached:
        return json.loads(cached)
    return None


def invalidate_preferences_cache(user_id: str):
    """Invalidate cached user preferences"""
    redis = get_redis()
    key = f"user_preferences:{user_id}"
    redis.delete(key)

