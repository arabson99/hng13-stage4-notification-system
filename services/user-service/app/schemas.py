from pydantic import BaseModel, EmailStr, Field
from typing import Optional, List
from datetime import datetime


# User Schemas
class UserBase(BaseModel):
    username: str = Field(..., min_length=3, max_length=100)
    email: EmailStr


class UserCreate(UserBase):
    password: str = Field(..., min_length=6)


class UserUpdate(BaseModel):
    username: Optional[str] = Field(None, min_length=3, max_length=100)
    email: Optional[EmailStr] = None
    push_token: Optional[str] = None
    is_active: Optional[bool] = None


class UserResponse(UserBase):
    id: int
    push_token: Optional[str] = None
    is_active: bool
    created_at: datetime
    updated_at: Optional[datetime] = None

    class Config:
        from_attributes = True


# Notification Preference Schemas
class NotificationPreferenceBase(BaseModel):
    notification_type: str = Field(..., description="Type: email, push, sms")
    channel: str = Field(..., description="Channel: marketing, transactional, etc.")
    enabled: bool = True


class NotificationPreferenceCreate(NotificationPreferenceBase):
    user_id: int


class NotificationPreferenceUpdate(BaseModel):
    enabled: Optional[bool] = None


class NotificationPreferenceResponse(NotificationPreferenceBase):
    id: int
    user_id: int
    created_at: datetime
    updated_at: Optional[datetime] = None

    class Config:
        from_attributes = True


# Auth Schemas
class Token(BaseModel):
    access_token: str
    token_type: str = "bearer"


class TokenData(BaseModel):
    username: Optional[str] = None


class LoginRequest(BaseModel):
    username: str
    password: str


# Health Check Schema
class HealthResponse(BaseModel):
    status: str
    service: str
    database: str
    redis: str

