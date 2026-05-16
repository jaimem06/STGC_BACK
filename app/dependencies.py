     2 """FastAPI dependencies for authentication,
       authorization, and auditing"""
     3
     4 from typing import Annotated, Callable
     5
     6 from fastapi import Depends, HTTPException,
       Request, status
     7 from fastapi.security import OAuth2PasswordBearer
     8 from sqlalchemy.ext.asyncio import AsyncSession
     9 from sqlalchemy import select
    10
    11 from app.security import decode_access_token
    12 from app.database import get_db
    13 from app.models.user import User, RoleEnum
    14 from app.models.audit import AuditLog
    15
    16
    17 oauth2_scheme =
       OAuth2PasswordBearer(tokenUrl="/api/auth/login")
    18
    19
    20 def get_client_ip(request: Request) -> str:
    21     """Get client IP address from request
       headers"""
    22     forwarded =
       request.headers.get("X-Forwarded-For")
    23     if forwarded:
    24         return forwarded.split(",")[0].strip()
    25     return request.client.host if request.client
       else "unknown"
    26
    27
    28 async def get_current_user(
    29     token: Annotated[str, Depends(oauth2_scheme)],
    30     db: Annotated[AsyncSession, Depends(get_db)],
    31 ) -> User:
    32     """
    33     Dependency to get current authenticated user
       and validate single session.
    34     """
    35     credentials_exception = HTTPException(
    36         status_code=status.HTTP_401_UNAUTHORIZED,
    37         detail="No se pudieron validar las
       credenciales",
    38         headers={"WWW-Authenticate": "Bearer"},
    39     )
    40
    41     session_invalidated_exception = HTTPException(
    42         status_code=status.HTTP_401_UNAUTHORIZED,
    43         detail="Sesión invalidada. Alguien más
       inició sesión con tu cuenta.",
    44         headers={"WWW-Authenticate": "Bearer"},
    45     )
    46
    47     payload = decode_access_token(token)
    48     if payload is None:
    49         raise credentials_exception
    50
    51     user_id: str = payload.get("sub")
    52     session_token: str =
       payload.get("session_token")
    53
    54     if not user_id or not session_token:
    55         raise credentials_exception
    56
    57     stmt = select(User).where(User.id == user_id)
    58     result = await db.execute(stmt)
    59     user = result.scalar_one_or_none()
    60
    61     if user is None:
    62         raise credentials_exception
    63
    64     if user.session_token != session_token:
    65         raise session_invalidated_exception
    66
    67     return user
    68
    69
    async def get_current_active_user(
        current_user: Annotated[User, Depends(get_current_user)],
    ) -> User:
        """Dependency to ensure user is active"""
        if not current_user.is_active:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="Usuario inactivo",
            )
        return current_user


    class RoleChecker:
        """Checker for user roles with inheritance for ADMIN and GERENTE_GENERAL"""

        def __init__(self, allowed_roles: list[RoleEnum]):
            self.allowed_roles = allowed_roles

        def __call__(
            self,
            current_user: Annotated[User, Depends(get_current_active_user)],
        ) -> User:
            # ADMIN and GERENTE_GENERAL have access to everything
            if current_user.role in [RoleEnum.ADMIN, RoleEnum.GERENTE_GENERAL]:
                return current_user

            if current_user.role not in self.allowed_roles:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail="No tienes permisos suficientes para realizar esta acción",
                )
            return current_user


    # Role-specific dependency instances
    require_admin = RoleChecker([RoleEnum.ADMIN])
    require_productor = RoleChecker([RoleEnum.PRODUCTOR])
    require_catador = RoleChecker([RoleEnum.CATADOR])
    require_barista = RoleChecker([RoleEnum.BARISTA])
    require_capataz = RoleChecker([RoleEnum.CAPATAZ])
    require_tostador = RoleChecker([RoleEnum.TOSTADOR])
    require_gestor_calidad = RoleChecker([RoleEnum.GESTOR_CALIDAD])


    def log_user_action(action: str) -> Callable:
        """
        Dependency factory to log user actions in the AuditLog.
        """
        async def _log_action(
            request: Request,
            current_user: Annotated[User, Depends(get_current_user)],
            db: Annotated[AsyncSession, Depends(get_db)],
        ):
            ip_address = get_client_ip(request)
            audit_entry = AuditLog(
                user_id=current_user.id,
                action=action,
                endpoint=str(request.url.path),
                ip_address=ip_address,
            )
            db.add(audit_entry)
            await db.commit()
            return None

        return _log_action

