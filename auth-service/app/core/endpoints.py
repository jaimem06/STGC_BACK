"""
Centralized API endpoint paths for the application.
Use these constants in routers to avoid hardcoded strings.
"""

# Auth Router Prefix
AUTH_PREFIX = "/auth"

# Auth Endpoints
AUTH_REGISTER = "/register"
AUTH_LOGIN = "/login"
AUTH_ME = "/me"
AUTH_RECOVERY = "/password-recovery"
AUTH_RESET = "/reset-password"

# Roles Router Prefix
ROLES_PREFIX = "/roles"

# Roles Endpoints
ROLES_LIST = "/"
ROLES_CREATE = "/"
ROLES_UPDATE = "/{role_id}"
ROLES_DELETE = "/{role_id}"

# Users Router Prefix
USERS_PREFIX = "/users"

# Users Endpoints
USERS_LIST = "/"
USERS_UPDATE = "/{user_id}"
USERS_SUSPEND = "/{user_id}/suspend"

# Global / Health
HEALTH_CHECK = "/"
DOCS = "/docs"
ADMIN_ONLY_TEST = "/admin-only"
