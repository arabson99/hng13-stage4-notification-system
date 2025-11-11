from fastapi import FastAPI, Depends, HTTPException, status, Path
from fastapi.middleware.cors import CORSMiddleware
from sqlalchemy.orm import Session
from sqlalchemy import text
from typing import List
from app.database import get_db, init_db
from app.models import User, NotificationPreference
from app.schemas import (
    UserCreate, UserUpdate, UserResponse,
    NotificationPreferenceCreate, NotificationPreferenceUpdate, NotificationPreferenceResponse,
    LoginRequest, Token, HealthResponse
)
from app.auth import create_access_token, get_current_active_user
from app.config import settings
from app.redis_client import (
    cache_user_preferences, get_cached_preferences,
    invalidate_preferences_cache, get_redis
)
from datetime import timedelta
import redis

app = FastAPI(
    title="User Service",
    description="User management and notification preferences service",
    version="1.0.0"
)

# CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.on_event("startup")
async def startup_event():
    """Initialize database on startup"""
    init_db()


# Health Check Endpoint
@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint"""
    health_status = {
        "status": "healthy",
        "service": settings.service_name,
        "database": "unknown",
        "redis": "unknown"
    }
    
    # Check database connection
    try:
        db = next(get_db())
        db.execute(text("SELECT 1"))
        health_status["database"] = "connected"
        db.close()
    except Exception as e:
        health_status["database"] = f"error: {str(e)}"
        health_status["status"] = "unhealthy"
    
    # Check Redis connection
    try:
        redis_client = get_redis()
        redis_client.ping()
        health_status["redis"] = "connected"
    except Exception as e:
        health_status["redis"] = f"error: {str(e)}"
        health_status["status"] = "unhealthy"
    
    return health_status


# Authentication Endpoints
@app.post("/login", response_model=Token)
async def login(login_data: LoginRequest, db: Session = Depends(get_db)):
    """User login endpoint"""
    user = db.query(User).filter(User.username == login_data.username).first()
    
    if not user or not user.verify_password(login_data.password):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
            headers={"WWW-Authenticate": "Bearer"},
        )
    
    if not user.is_active:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Inactive user"
        )
    
    access_token_expires = timedelta(minutes=settings.access_token_expire_minutes)
    access_token = create_access_token(
        data={"sub": user.username}, expires_delta=access_token_expires
    )
    
    return {"access_token": access_token, "token_type": "bearer"}


# User CRUD Endpoints
@app.post("/users", response_model=UserResponse, status_code=status.HTTP_201_CREATED)
async def create_user(user: UserCreate, db: Session = Depends(get_db)):
    """Create a new user"""
    # Check if username exists
    db_user = db.query(User).filter(User.username == user.username).first()
    if db_user:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Username already registered"
        )
    
    # Check if email exists
    db_user = db.query(User).filter(User.email == user.email).first()
    if db_user:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Email already registered"
        )
    
    new_user = User(
        username=user.username,
        email=user.email
    )
    new_user.set_password(user.password)
    
    db.add(new_user)
    db.commit()
    db.refresh(new_user)
    
    return new_user


@app.get("/users", response_model=List[UserResponse])
async def get_users(
    skip: int = 0,
    limit: int = 100,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Get all users (requires authentication)"""
    users = db.query(User).offset(skip).limit(limit).all()
    return users


@app.get("/users/me", response_model=UserResponse)
async def get_current_user_info(
    current_user: User = Depends(get_current_active_user)
):
    """Get current authenticated user info"""
    return current_user


@app.get("/users/{user_id}", response_model=UserResponse)
async def get_user_flexible(
    user_id: str = Path(..., description="User ID or 'me' for current user"),
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """
    Get a user by ID or the current authenticated user if 'me' is used.
    """
    if user_id == "me":
        return current_user
    
    try:
        user_id_int = int(user_id)
    except ValueError:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="user_id must be an integer or 'me'"
        )
    
    user = db.query(User).filter(User.id == user_id_int).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    
    return user


@app.put("/users/{user_id}", response_model=UserResponse)
async def update_user(
    user_id: int,
    user_update: UserUpdate,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Update user (users can only update themselves unless admin)"""
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    
    # Users can only update themselves
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    # Check if new username/email already exists
    if user_update.username and user_update.username != user.username:
        existing_user = db.query(User).filter(User.username == user_update.username).first()
        if existing_user:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Username already taken"
            )
        user.username = user_update.username
    
    if user_update.email and user_update.email != user.email:
        existing_user = db.query(User).filter(User.email == user_update.email).first()
        if existing_user:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Email already taken"
            )
        user.email = user_update.email
    
    if user_update.push_token is not None:
        user.push_token = user_update.push_token
    
    if user_update.is_active is not None:
        user.is_active = user_update.is_active
    
    db.commit()
    db.refresh(user)
    
    return user


@app.delete("/users/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_user(
    user_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Delete user"""
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    
    # Users can only delete themselves
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    # Invalidate cache
    invalidate_preferences_cache(str(user_id))
    
    db.delete(user)
    db.commit()
    
    return None


# Notification Preferences CRUD Endpoints
@app.post("/users/{user_id}/preferences", response_model=NotificationPreferenceResponse, status_code=status.HTTP_201_CREATED)
async def create_preference(
    user_id: int,
    preference: NotificationPreferenceCreate,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Create notification preference for a user"""
    # Verify user exists
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    
    # Users can only create preferences for themselves
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    # Check if preference already exists
    existing = db.query(NotificationPreference).filter(
        NotificationPreference.user_id == user_id,
        NotificationPreference.notification_type == preference.notification_type,
        NotificationPreference.channel == preference.channel
    ).first()
    
    if existing:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Preference already exists"
        )
    
    new_preference = NotificationPreference(
        user_id=user_id,
        notification_type=preference.notification_type,
        channel=preference.channel,
        enabled=preference.enabled
    )
    
    db.add(new_preference)
    db.commit()
    db.refresh(new_preference)
    
    # Invalidate cache
    invalidate_preferences_cache(str(user_id))
    
    return new_preference


@app.get("/users/{user_id}/preferences", response_model=List[NotificationPreferenceResponse])
async def get_preferences(
    user_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Get all notification preferences for a user"""
    # Verify user exists
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    
    # Users can only view their own preferences
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    # Get from database
    preferences = db.query(NotificationPreference).filter(
        NotificationPreference.user_id == user_id
    ).all()
    
    # Cache the preferences as a list of dicts
    preferences_list = []
    for pref in preferences:
        pref_dict = {
            "id": pref.id,
            "user_id": pref.user_id,
            "notification_type": pref.notification_type,
            "channel": pref.channel,
            "enabled": pref.enabled,
            "created_at": pref.created_at.isoformat() if pref.created_at else None,
            "updated_at": pref.updated_at.isoformat() if pref.updated_at else None
        }
        preferences_list.append(pref_dict)
    
    cache_user_preferences(str(user_id), preferences_list)
    
    return preferences


@app.get("/users/{user_id}/preferences/{preference_id}", response_model=NotificationPreferenceResponse)
async def get_preference(
    user_id: int,
    preference_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Get a specific notification preference"""
    preference = db.query(NotificationPreference).filter(
        NotificationPreference.id == preference_id,
        NotificationPreference.user_id == user_id
    ).first()
    
    if not preference:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Preference not found"
        )
    
    # Users can only view their own preferences
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    return preference


@app.put("/users/{user_id}/preferences/{preference_id}", response_model=NotificationPreferenceResponse)
async def update_preference(
    user_id: int,
    preference_id: int,
    preference_update: NotificationPreferenceUpdate,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Update a notification preference"""
    preference = db.query(NotificationPreference).filter(
        NotificationPreference.id == preference_id,
        NotificationPreference.user_id == user_id
    ).first()
    
    if not preference:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Preference not found"
        )
    
    # Users can only update their own preferences
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    if preference_update.enabled is not None:
        preference.enabled = preference_update.enabled
    
    db.commit()
    db.refresh(preference)
    
    # Invalidate cache
    invalidate_preferences_cache(str(user_id))
    
    return preference


@app.delete("/users/{user_id}/preferences/{preference_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_preference(
    user_id: int,
    preference_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_active_user)
):
    """Delete a notification preference"""
    preference = db.query(NotificationPreference).filter(
        NotificationPreference.id == preference_id,
        NotificationPreference.user_id == user_id
    ).first()
    
    if not preference:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Preference not found"
        )
    
    # Users can only delete their own preferences
    if current_user.id != user_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not enough permissions"
        )
    
    db.delete(preference)
    db.commit()
    
    # Invalidate cache
    invalidate_preferences_cache(str(user_id))
    
    return None


# Additional endpoint for API Gateway to get user info by ID (no auth required for gateway)
@app.get("/internal/users/{user_id}", response_model=UserResponse)
async def get_user_internal(user_id: int, db: Session = Depends(get_db)):
    """Internal endpoint for API Gateway to get user info"""
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found"
        )
    return user


# Endpoint for API Gateway to get cached preferences
@app.get("/internal/users/{user_id}/preferences", response_model=List[NotificationPreferenceResponse])
async def get_preferences_internal(user_id: int, db: Session = Depends(get_db)):
    """Internal endpoint for API Gateway to get user preferences (with caching)"""
    # Get from database
    preferences = db.query(NotificationPreference).filter(
        NotificationPreference.user_id == user_id
    ).all()
    
    # Cache the preferences as a list of dicts
    preferences_list = []
    for pref in preferences:
        pref_dict = {
            "id": pref.id,
            "user_id": pref.user_id,
            "notification_type": pref.notification_type,
            "channel": pref.channel,
            "enabled": pref.enabled,
            "created_at": pref.created_at.isoformat() if pref.created_at else None,
            "updated_at": pref.updated_at.isoformat() if pref.updated_at else None
        }
        preferences_list.append(pref_dict)
    
    cache_user_preferences(str(user_id), preferences_list)
    
    return preferences

