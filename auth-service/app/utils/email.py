import html
import logging
import re
import smtplib
from email.mime.multipart import MIMEMultipart
from email.mime.text import MIMEText
from email.utils import formatdate, make_msgid
from typing import Optional
from urllib.parse import quote

from app.config import settings

logger = logging.getLogger(__name__)


def clean_html(raw_html: str) -> str:
    """Extrae texto plano de HTML de forma básica."""
    text = re.sub(r"<(style|script)[^>]*>.*?</\1>", "", raw_html, flags=re.DOTALL | re.IGNORECASE)
    text = re.sub(r"<a [^>]*href=\"([^\"]+)\"[^>]*>(.*?)</a>", r"\2: \1", text, flags=re.DOTALL)
    text = re.sub(r"<.*?>", "", text, flags=re.DOTALL)
    return re.sub(r"\n\s*\n+", "\n\n", html.unescape(text)).strip()


def send_email(recipient_email: str, subject: str, body_html: str) -> bool:
    """
    Envía un correo por SMTP (TLS en 587, SSL en 465).
    Devuelve True si se envió. Nunca lanza excepciones (pensado para BackgroundTasks).
    """
    if not settings.smtp_user or not settings.smtp_password:
        logger.error("SMTP no configurado: el correo a %s NO fue enviado.", recipient_email)
        return False

    message = MIMEMultipart("alternative")
    message["Subject"] = subject
    message["From"] = settings.smtp_from_email or settings.smtp_user
    message["To"] = recipient_email
    message["Date"] = formatdate(localtime=False)
    message["Message-ID"] = make_msgid()
    message.attach(MIMEText(clean_html(body_html), "plain", "utf-8"))
    message.attach(MIMEText(body_html, "html", "utf-8"))

    try:
        server_class = smtplib.SMTP_SSL if settings.smtp_port == 465 else smtplib.SMTP
        with server_class(settings.smtp_host, settings.smtp_port, timeout=10) as server:
            server.ehlo()
            if settings.smtp_port != 465 and settings.smtp_tls:
                server.starttls()
                server.ehlo()
            server.login(settings.smtp_user, settings.smtp_password)
            server.send_message(message)
        logger.info("Correo '%s' enviado a %s", subject, recipient_email)
        return True
    except smtplib.SMTPAuthenticationError as e:
        logger.error(
            "Error de autenticación SMTP para %r (código %s). Revisa SMTP_USER/SMTP_PASSWORD. "
            "El correo a %s NO fue enviado.",
            settings.smtp_user, getattr(e, "smtp_code", "?"), recipient_email,
        )
    except (smtplib.SMTPConnectError, smtplib.SMTPServerDisconnected, TimeoutError, OSError) as e:
        logger.error(
            "Error de conexión SMTP a %s:%s (%s). El correo a %s NO fue enviado.",
            settings.smtp_host, settings.smtp_port, type(e).__name__, recipient_email,
        )
    except Exception as e:
        logger.error("Error inesperado al enviar correo a %s: %s", recipient_email, type(e).__name__)
    return False


def _render(title: str, paragraphs: list[str], button_text: Optional[str] = None,
            link: Optional[str] = None, footer_note: Optional[str] = None) -> str:
    """Plantilla HTML común. Todos los textos se escapan antes de insertarse."""
    app_name = html.escape(settings.app_display_name)
    body = "".join(f"<p>{html.escape(p)}</p>" for p in paragraphs)
    button = ""
    if button_text and link:
        safe_link = html.escape(link, quote=True)
        button = f"""
                <div style="text-align: center; margin: 36px 0;">
                    <a href="{safe_link}"
                       style="background-color: #2563eb; color: #ffffff; padding: 14px 28px; text-decoration: none; border-radius: 8px; font-weight: bold; display: inline-block;">
                       {html.escape(button_text)}
                    </a>
                </div>
                <p style="font-size: 0.85em; color: #52606d;">Si el botón no funciona, copia este enlace en tu navegador:<br>{safe_link}</p>"""
    note = f'<p style="font-size: 0.9em; color: #52606d;">{html.escape(footer_note)}</p>' if footer_note else ""
    return f"""
    <html>
        <body style="font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; line-height: 1.6; background-color: #f4f5f7; padding: 20px; color: #1f2933;">
            <div style="max-width: 600px; margin: auto; background-color: #ffffff; border: 1px solid #d9dde3; padding: 40px; border-radius: 12px;">
                <h1 style="margin: 0 0 24px; font-size: 22px; text-align: center;">{app_name}</h1>
                <h2 style="color: #2563eb; font-size: 18px;">{html.escape(title)}</h2>
                {body}
                {button}
                {note}
                <hr style="border: 0; border-top: 1px solid #d9dde3; margin: 30px 0;">
                <p style="font-size: 0.8em; color: #52606d; text-align: center;">
                    {app_name}<br>Este es un correo automático, por favor no responda.
                </p>
            </div>
        </body>
    </html>
    """


def _link(path: str, token: str) -> str:
    return f"{settings.frontend_link(path)}?token={quote(token, safe='')}"


def send_password_reset_email(email: str, token: str) -> bool:
    minutes = settings.password_reset_token_expire_minutes
    body = _render(
        "Restablecer contraseña",
        [
            "Hola,",
            f"Recibimos una solicitud para restablecer tu contraseña en {settings.app_display_name}.",
            f"El enlace caduca en {minutes} minutos y solo puede usarse una vez.",
        ],
        "CREAR NUEVA CONTRASEÑA",
        _link(settings.frontend_reset_password_path, token),
        "¿No solicitaste este cambio? Ignora este correo: tu contraseña actual no cambiará.",
    )
    return send_email(email, f"Recuperación de contraseña - {settings.app_display_name}", body)


def send_invitation_email(email: str, token: str) -> bool:
    hours = settings.invitation_token_expire_hours
    body = _render(
        "Te han creado una cuenta",
        [
            "Hola,",
            f"Se ha creado una cuenta para ti en {settings.app_display_name}.",
            f"Define tu contraseña con el siguiente enlace. Caduca en {hours} horas.",
        ],
        "DEFINIR CONTRASEÑA",
        _link(settings.frontend_reset_password_path, token),
    )
    return send_email(email, f"Bienvenido a {settings.app_display_name}", body)


def send_verification_email(email: str, token: str) -> bool:
    hours = settings.email_verification_token_expire_hours
    body = _render(
        "Confirma tu correo electrónico",
        [
            "Hola,",
            f"Confirma tu dirección de correo para activar tu cuenta en {settings.app_display_name}.",
            f"El enlace caduca en {hours} horas.",
        ],
        "CONFIRMAR CORREO",
        _link(settings.frontend_verify_email_path, token),
        "Si no creaste esta cuenta, ignora este correo.",
    )
    return send_email(email, f"Confirma tu correo - {settings.app_display_name}", body)


def send_password_changed_email(email: str) -> bool:
    body = _render(
        "Tu contraseña ha cambiado",
        [
            "Hola,",
            f"La contraseña de tu cuenta en {settings.app_display_name} se cambió correctamente "
            "y se cerraron las demás sesiones abiertas.",
        ],
        footer_note="Si no fuiste tú, restablece tu contraseña de inmediato y contacta al administrador.",
    )
    return send_email(email, f"Contraseña actualizada - {settings.app_display_name}", body)


def send_mfa_changed_email(email: str, enabled: bool) -> bool:
    state = "activó" if enabled else "desactivó"
    body = _render(
        "Verificación en dos pasos",
        ["Hola,", f"Se {state} la verificación en dos pasos de tu cuenta en {settings.app_display_name}."],
        footer_note="Si no fuiste tú, cambia tu contraseña de inmediato y contacta al administrador.",
    )
    return send_email(email, f"Seguridad de tu cuenta - {settings.app_display_name}", body)
