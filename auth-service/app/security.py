"""
Utilidades criptográficas: contraseñas, JWT, tokens opacos, TOTP y cifrado.
"""

import base64
import hashlib
import hmac
import secrets
import struct
import time
import uuid
from datetime import datetime, timedelta, timezone
from functools import lru_cache
from typing import Any, Optional
from urllib.parse import quote

import bcrypt
import jwt
from cryptography.fernet import Fernet, InvalidToken
from cryptography.hazmat.primitives import serialization

from app.config import settings

# ---------------------------------------------------------------------------
# Contraseñas
# ---------------------------------------------------------------------------

# bcrypt solo procesa los primeros 72 bytes; más allá se rechaza explícitamente.
BCRYPT_MAX_BYTES = 72
BCRYPT_ROUNDS = 12

# Hash fijo usado para igualar tiempos cuando el usuario no existe (anti-enumeración).
_DUMMY_HASH = bcrypt.hashpw(b"timing-equalizer-password", bcrypt.gensalt(BCRYPT_ROUNDS))

COMMON_PASSWORDS = {
    "password", "password1", "password123", "passw0rd", "contraseña", "contrasena",
    "contrasena1", "123456", "1234567", "12345678", "123456789", "1234567890",
    "qwerty", "qwerty123", "qwertyuiop", "abc123", "abc12345", "admin", "admin123",
    "admin1234", "administrator", "letmein", "welcome", "welcome1", "iloveyou",
    "monkey", "dragon", "football", "baseball", "sunshine", "princess", "master",
    "123123", "111111", "000000", "1q2w3e4r", "1q2w3e4r5t", "zaq12wsx", "changeme",
    "secret", "secreto", "usuario", "usuario1", "teamo", "hola123", "holamundo",
}


def get_password_hash(password: str) -> str:
    data = password.encode("utf-8")
    if len(data) > BCRYPT_MAX_BYTES:
        raise ValueError("La contraseña supera el tamaño máximo permitido")
    return bcrypt.hashpw(data, bcrypt.gensalt(BCRYPT_ROUNDS)).decode("utf-8")


def verify_password(plain_password: str, hashed_password: str) -> bool:
    data = plain_password.encode("utf-8")
    if len(data) > BCRYPT_MAX_BYTES:
        return False
    try:
        return bcrypt.checkpw(data, hashed_password.encode("utf-8"))
    except ValueError:
        return False


def burn_password_check(plain_password: str) -> None:
    """Ejecuta un bcrypt equivalente para que un email inexistente tarde lo mismo."""
    bcrypt.checkpw(plain_password.encode("utf-8")[:BCRYPT_MAX_BYTES], _DUMMY_HASH)


def unusable_password_hash() -> str:
    """Hash de una contraseña aleatoria desconocida (cuentas creadas por invitación)."""
    return get_password_hash(secrets.token_urlsafe(32))


def password_policy_errors(password: str, email: Optional[str] = None) -> list[str]:
    """Valida la contraseña según la política configurada. Lista vacía = válida."""
    errors: list[str] = []
    if len(password) < settings.password_min_length:
        errors.append(f"Debe tener al menos {settings.password_min_length} caracteres")
    if len(password.encode("utf-8")) > BCRYPT_MAX_BYTES:
        errors.append(f"No puede superar {BCRYPT_MAX_BYTES} bytes")
    if settings.password_require_complexity:
        if not any(c.isalpha() for c in password) or not any(c.isdigit() for c in password):
            errors.append("Debe contener al menos una letra y un número")
    lowered = password.lower()
    if lowered in COMMON_PASSWORDS:
        errors.append("Es una contraseña demasiado común")
    if email:
        local = email.split("@")[0].lower()
        if lowered == email.lower() or (len(local) >= 4 and local in lowered):
            errors.append("No debe contener tu email")
    if len(set(password)) < 3:
        errors.append("Tiene demasiados caracteres repetidos")
    return errors


# ---------------------------------------------------------------------------
# JWT
# ---------------------------------------------------------------------------

ACCESS_TOKEN_TYPE = "access"
MFA_TOKEN_TYPE = "mfa"


@lru_cache(maxsize=1)
def _rsa_keys():
    private_key = serialization.load_pem_private_key(
        settings.jwt_private_key.encode(), password=None
    )
    public_key = serialization.load_pem_public_key(settings.jwt_public_key.encode())
    public_pem = public_key.public_bytes(
        serialization.Encoding.PEM, serialization.PublicFormat.SubjectPublicKeyInfo
    )
    kid = hashlib.sha256(public_pem).hexdigest()[:16]
    return private_key, public_key, kid


def _signing_key():
    return _rsa_keys()[0] if settings.uses_asymmetric_jwt else settings.secret_key


def _verification_key():
    return _rsa_keys()[1] if settings.uses_asymmetric_jwt else settings.secret_key


def _encode(claims: dict[str, Any]) -> str:
    headers = {"kid": _rsa_keys()[2]} if settings.uses_asymmetric_jwt else None
    return jwt.encode(claims, _signing_key(), algorithm=settings.jwt_algorithm, headers=headers)


def _base_claims(subject: str, token_type: str, lifetime: timedelta) -> dict[str, Any]:
    now = datetime.now(timezone.utc)
    return {
        "iss": settings.jwt_issuer,
        "aud": settings.jwt_audience,
        "sub": subject,
        "type": token_type,
        "iat": now,
        "nbf": now,
        "exp": now + lifetime,
        "jti": str(uuid.uuid4()),
    }


def create_access_token(user_id: str, role: str, session_id: str) -> tuple[str, int]:
    """Crea el access token. Devuelve (token, segundos_de_vida)."""
    lifetime = timedelta(minutes=settings.access_token_expire_minutes)
    claims = _base_claims(user_id, ACCESS_TOKEN_TYPE, lifetime)
    claims.update({"role": role, "sid": session_id})
    return _encode(claims), int(lifetime.total_seconds())


def create_mfa_token(user_id: str) -> str:
    """Token intermedio tras validar la contraseña de un usuario con MFA activo."""
    lifetime = timedelta(minutes=settings.mfa_token_expire_minutes)
    return _encode(_base_claims(user_id, MFA_TOKEN_TYPE, lifetime))


def decode_token(token: str, expected_type: str) -> Optional[dict[str, Any]]:
    """
    Verifica firma, algoritmo, expiración, emisor, audiencia y tipo de token.
    Devuelve los claims o None si algo no es válido.
    """
    try:
        payload = jwt.decode(
            token,
            _verification_key(),
            algorithms=[settings.jwt_algorithm],
            audience=settings.jwt_audience,
            issuer=settings.jwt_issuer,
            options={"require": ["exp", "iat", "sub", "type", "iss", "aud"]},
        )
    except jwt.PyJWTError:
        return None
    if payload.get("type") != expected_type:
        return None
    return payload


def get_jwks() -> Optional[dict[str, Any]]:
    """JWKS con la clave pública (solo con algoritmos RS*)."""
    if not settings.uses_asymmetric_jwt:
        return None
    _, public_key, kid = _rsa_keys()
    jwk = jwt.algorithms.RSAAlgorithm.to_jwk(public_key, as_dict=True)
    jwk.update({"kid": kid, "use": "sig", "alg": settings.jwt_algorithm})
    return {"keys": [jwk]}


# ---------------------------------------------------------------------------
# Tokens opacos (refresh, recuperación, verificación)
# ---------------------------------------------------------------------------

def generate_opaque_token() -> str:
    return secrets.token_urlsafe(32)


def hash_token(token: str) -> str:
    """Los tokens opacos se guardan solo como SHA-256: un volcado de la BD no los expone."""
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def constant_time_equals(a: Optional[str], b: Optional[str]) -> bool:
    if not a or not b:
        return False
    return hmac.compare_digest(a.encode("utf-8"), b.encode("utf-8"))


# ---------------------------------------------------------------------------
# Cifrado de secretos en reposo
# ---------------------------------------------------------------------------

@lru_cache(maxsize=1)
def _fernet() -> Fernet:
    if settings.data_encryption_key:
        return Fernet(settings.data_encryption_key.encode())
    derived = hashlib.sha256(f"data-encryption:{settings.secret_key}".encode()).digest()
    return Fernet(base64.urlsafe_b64encode(derived))


def encrypt_secret(value: str) -> str:
    return _fernet().encrypt(value.encode()).decode()


def decrypt_secret(value: str) -> Optional[str]:
    try:
        return _fernet().decrypt(value.encode()).decode()
    except (InvalidToken, ValueError):
        return None


# ---------------------------------------------------------------------------
# TOTP (RFC 6238) compatible con Google Authenticator, Authy, 1Password, etc.
# ---------------------------------------------------------------------------

TOTP_PERIOD = 30
TOTP_DIGITS = 6


def generate_totp_secret() -> str:
    return base64.b32encode(secrets.token_bytes(20)).decode().rstrip("=")


def _b32decode(secret: str) -> bytes:
    secret = secret.strip().replace(" ", "").upper()
    return base64.b32decode(secret + "=" * (-len(secret) % 8))


def hotp(key: bytes, counter: int, digits: int = TOTP_DIGITS) -> str:
    digest = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
    offset = digest[-1] & 0x0F
    code = struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF
    return str(code % (10 ** digits)).zfill(digits)


def verify_totp(
    secret: str,
    code: str,
    last_used_step: Optional[int] = None,
    window: int = 1,
) -> Optional[int]:
    """
    Verifica un código TOTP tolerando ±window periodos de desfase de reloj.
    Rechaza códigos ya usados (step <= last_used_step) para impedir su reutilización.
    Devuelve el step aceptado o None.
    """
    code = (code or "").strip().replace(" ", "")
    if len(code) != TOTP_DIGITS or not code.isdigit():
        return None
    key = _b32decode(secret)
    current = int(time.time() // TOTP_PERIOD)
    for step in range(current - window, current + window + 1):
        if last_used_step is not None and step <= last_used_step:
            continue
        if hmac.compare_digest(hotp(key, step), code):
            return step
    return None


def totp_provisioning_uri(secret: str, account: str, issuer: str) -> str:
    label = quote(f"{issuer}:{account}")
    return (
        f"otpauth://totp/{label}?secret={secret}&issuer={quote(issuer)}"
        f"&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_PERIOD}"
    )
