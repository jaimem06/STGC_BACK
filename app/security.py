 """Security utilities: JWT, password hashing, and
      session tokens"""
    3
    4 import uuid
    5 from datetime import datetime, timedelta, timezone
    6 from typing import Any, Optional
    7
    8 import bcrypt
    9 from jose import JWTError, jwt
   10
   11 from app.config import settings
   12
   13
   14 def verify_password(plain_password: str,
      hashed_password: str) -> bool:
   15     """Verify a plain password against its hash
      using bcrypt"""
   16     return bcrypt.checkpw(
   17         plain_password.encode("utf-8"),
   18         hashed_password.encode("utf-8"),
   19     )
   20
   21
   22 def get_password_hash(password: str) -> str:
   23     """Generate BCrypt hash of password"""
   24     salt = bcrypt.gensalt()
   25     hashed = bcrypt.hashpw(password.encode("utf-8"),
      salt)
   26     return hashed.decode("utf-8")
   27
   28
   29 def generate_session_token() -> str:
   30     """Generate unique session token for
      single-session enforcement"""
   31     return str(uuid.uuid4())
   32
   33
   34 def create_access_token(
   35     data: dict[str, Any],
   36     expires_delta: Optional[timedelta] = None,
   37 ) -> str:
   38     """
   39     Create JWT access token with session_token
      embedded.
   40     """
   41     to_encode = data.copy()
   42
   43     if expires_delta:
   44         expire = datetime.now(timezone.utc) +
      expires_delta
   45     else:
   46         expire = datetime.now(timezone.utc) +
      timedelta(
   47
      minutes=settings.access_token_expire_minutes
   48         )
   49
   50     to_encode.update({"exp": expire})
   51
   52     return jwt.encode(
   53         to_encode,
   54         settings.secret_key,
   55         algorithm=settings.jwt_algorithm,
   56     )
   57
   58
   59 def decode_access_token(token: str) ->
      Optional[dict[str, Any]]:
   60     """
   61     Decode and verify JWT token.
   62     """
   63     try:
   64         return jwt.decode(
   65             token,
   66             settings.secret_key,
   67             algorithms=[settings.jwt_algorithm],
   68         )
   69     except JWTError:
   70         return None
