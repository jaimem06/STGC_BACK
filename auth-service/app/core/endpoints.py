"""
Rutas centralizadas de la API.
Usar estas constantes en los routers evita cadenas repetidas y facilita renombrar rutas.
"""

# --- Autenticación ---
AUTH_PREFIX = "/auth"
AUTH_REGISTER = "/register"
AUTH_SIGNUP = "/signup"
AUTH_LOGIN = "/login"
AUTH_MFA_VERIFY = "/mfa/verify"
AUTH_REFRESH = "/refresh"
AUTH_LOGOUT = "/logout"
AUTH_LOGOUT_ALL = "/logout-all"
AUTH_RECOVERY = "/password-recovery"
AUTH_RESET = "/reset-password"
AUTH_VERIFY_EMAIL = "/verify-email"
AUTH_RESEND_VERIFICATION = "/resend-verification"

# --- Cuenta propia (mismo prefijo /auth) ---
ACCOUNT_ME = "/me"
ACCOUNT_CHANGE_PASSWORD = "/change-password"
ACCOUNT_SESSIONS = "/sessions"
ACCOUNT_SESSION_DETAIL = "/sessions/{session_id}"
ACCOUNT_MFA_SETUP = "/mfa/setup"
ACCOUNT_MFA_ENABLE = "/mfa/enable"
ACCOUNT_MFA_DISABLE = "/mfa/disable"

# --- Roles ---
ROLES_PREFIX = "/roles"
ROLES_LIST = "/"
ROLES_CREATE = "/"
ROLES_DETAIL = "/{role_id}"

# --- Usuarios ---
USERS_PREFIX = "/users"
USERS_LIST = "/"
USERS_DETAIL = "/{user_id}"
USERS_SUSPEND = "/{user_id}/suspend"
USERS_UNLOCK = "/{user_id}/unlock"
USERS_REVOKE_SESSIONS = "/{user_id}/revoke-sessions"
USERS_RESET_MFA = "/{user_id}/mfa"

# --- Auditoría ---
AUDIT_PREFIX = "/audit-logs"
AUDIT_LIST = "/"

# --- Servicios internos (machine-to-machine) ---
INTERNAL_PREFIX = "/internal"
INTERNAL_AUDIT = "/audit"
INTERNAL_INTROSPECT = "/introspect"
INTERNAL_USER = "/users/{user_id}"

# --- Global ---
HEALTH_CHECK = "/"
HEALTH_READY = "/health/ready"
JWKS = "/.well-known/jwks.json"
DOCS = "/docs"
