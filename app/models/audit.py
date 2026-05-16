 """Audit log model for tracking user actions"""
    3
    4 from datetime import datetime, timezone
    5 from typing import Optional
    6
    7 from sqlalchemy import DateTime, ForeignKey, String
    8 from sqlalchemy.orm import Mapped, mapped_column
    9
   10 from app.database import Base, generate_uuid
   11
   12
   13 class AuditLog(Base):
   14     """Audit log model"""
   15
   16     __tablename__ = "audit_logs"
   17
   18     id: Mapped[str] = mapped_column(String(36),
      primary_key=True, default=generate_uuid)
   19     user_id: Mapped[str] = mapped_column(
   20         String(36), ForeignKey("users.id",
      ondelete="CASCADE"), nullable=False, index=True
   21     )
   22     action: Mapped[str] = mapped_column(String(255),
      nullable=False, index=True)
   23     endpoint: Mapped[Optional[str]] =
      mapped_column(String(255), nullable=True)
   24     ip_address: Mapped[Optional[str]] =
      mapped_column(String(45), nullable=True)
   25     
   26     created_at: Mapped[datetime] = mapped_column(
   27         DateTime(timezone=True), default=lambda:
      datetime.now(timezone.utc), nullable=False
   28     )
   29
   30     def __repr__(self) -> str:
   31         return f"<AuditLog {self.action} by User
      {self.user_id}>"
