"""User model with RoleEnum for STGC"""
    3
    4 from enum import Enum
    5 from typing import Optional
    6
    7 from sqlalchemy import Boolean, Enum as SQLEnum,
      String
    8 from sqlalchemy.orm import Mapped, mapped_column
    9
   10 from app.database import Base, generate_uuid
   11
   12
   13 class RoleEnum(str, Enum):
   14     """User roles based on system actors"""
   15     ADMIN = "ADMIN" # Añadido para compatibilidad
      con require_admin
   16     GERENTE_GENERAL = "GERENTE_GENERAL"
   17     GERENTE_OPERACIONES = "GERENTE_OPERACIONES"
   18     CAPATAZ = "CAPATAZ"
   19     SEMBRADOR = "SEMBRADOR"
   20     RECOLECTOR = "RECOLECTOR"
   21     CLASIFICADOR = "CLASIFICADOR"
   22     TECNICO_DESPULPADO = "TECNICO_DESPULPADO"
   23     ENCARGADO_SECADO = "ENCARGADO_SECADO"
   24     TOSTADOR = "TOSTADOR"
   25     GESTOR_CALIDAD = "GESTOR_CALIDAD"
   26     TECNICO_ALMACENAMIENTO =
      "TECNICO_ALMACENAMIENTO"
   27     CONTROLADOR_DESPACHO = "CONTROLADOR_DESPACHO"
   28     GESTOR_INVENTARIO = "GESTOR_INVENTARIO"
   29     PERSONAL_COCINA = "PERSONAL_COCINA"
   30     CAJERO_MESERO = "CAJERO_MESERO"
   31     # Roles legacy solicitados en las dependencias
   32     PRODUCTOR = "PRODUCTOR"
   33     CATADOR = "CATADOR"
   34     BARISTA = "BARISTA"
   35
   36
   37 class User(Base):
   38     """User model"""
   39
   40     __tablename__ = "users"
   41
   42     id: Mapped[str] = mapped_column(String(36),
      primary_key=True, default=generate_uuid)
   43     email: Mapped[str] = mapped_column(String(255),
      unique=True, nullable=False, index=True)
   44     password_hash: Mapped[str] =
      mapped_column(String(255), nullable=False)
   45
   46     role: Mapped[RoleEnum] = mapped_column(
   47         SQLEnum(RoleEnum),
      default=RoleEnum.CAJERO_MESERO, nullable=False
   48     )
   49     
   50     is_active: Mapped[bool] = mapped_column(Boolean,
      default=True, nullable=False)
   51     session_token: Mapped[Optional[str]] =
      mapped_column(String(36), nullable=True, index=True)
   52
   53     def __repr__(self) -> str:
   54         return f"<User {self.email}
      ({self.role.value})>"
