from sqlalchemy import Column, Integer, String, Boolean, DateTime, ForeignKey, Text
from sqlalchemy.orm import relationship
from sqlalchemy.sql import func
from app.database import Base
import bcrypt


class User(Base):
    __tablename__ = "users"

    id = Column(Integer, primary_key=True, index=True)
    username = Column(String(100), unique=True, index=True, nullable=False)
    email = Column(String(255), unique=True, index=True, nullable=False)
    hashed_password = Column(String(255), nullable=False)
    push_token = Column(Text, nullable=True)
    is_active = Column(Boolean, default=True)
    created_at = Column(DateTime(timezone=True), server_default=func.now())
    updated_at = Column(DateTime(timezone=True), onupdate=func.now())

    notification_preferences = relationship(
        "NotificationPreference", back_populates="user", cascade="all, delete-orphan"
    )

    def verify_password(self, password: str) -> bool:
        """Verify password against hashed password"""
        try:
            return bcrypt.checkpw(password.encode('utf-8'), self.hashed_password.encode('utf-8'))
        except Exception:
            return False

    def set_password(self, password: str):
        """Hash and set password"""
        hashed = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt())
        self.hashed_password = hashed.decode('utf-8')


class NotificationPreference(Base):
    __tablename__ = "notification_preferences"

    id = Column(Integer, primary_key=True, index=True)
    user_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), nullable=False, index=True)
    notification_type = Column(String(50), nullable=False)  # email, push, sms
    channel = Column(String(50), nullable=False)  # marketing, transactional, etc.
    enabled = Column(Boolean, default=True)
    created_at = Column(DateTime(timezone=True), server_default=func.now())
    updated_at = Column(DateTime(timezone=True), onupdate=func.now())

    user = relationship("User", back_populates="notification_preferences")

    __table_args__ = (
        {'unique_constraint': ('user_id', 'notification_type', 'channel')},
    )

