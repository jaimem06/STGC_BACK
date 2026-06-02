import smtplib
import logging
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from typing import List, Optional
import re
from app.config import settings

logger = logging.getLogger(__name__)

def clean_html(raw_html: str) -> str:
    """Extrae texto plano de HTML de forma básica."""
    cleanr = re.compile('<.*?>', re.DOTALL)
    cleantext = re.sub(cleanr, '', raw_html)
    return cleantext

def send_email(
    recipient_email: str,
    subject: str,
    body_html: str
):
    """
    Envía un correo electrónico utilizando la configuración SMTP.
    Soporta TLS (587) y SSL (465).
    """
    if not settings.smtp_user or not settings.smtp_password:
        logger.warning("SMTP no configurado. El correo no será enviado.")
        return

    message = MIMEMultipart("alternative")
    message["Subject"] = subject
    from_email = settings.smtp_from_email or settings.smtp_user
    message["From"] = from_email
    message["To"] = recipient_email

    # Versión en texto plano para mejor entregabilidad
    text_content = clean_html(body_html)
    message.attach(MIMEText(text_content, "plain", "utf-8"))
    message.attach(MIMEText(body_html, "html", "utf-8"))

    try:
        # Decidir entre SMTP estándar (con STARTTLS) o SMTP_SSL
        if settings.smtp_port == 465:
            server_class = smtplib.SMTP_SSL
        else:
            server_class = smtplib.SMTP

        with server_class(settings.smtp_host, settings.smtp_port, timeout=10) as server:
            server.ehlo()
            if settings.smtp_port != 465 and settings.smtp_tls:
                server.starttls()
                server.ehlo()
            
            server.login(settings.smtp_user, settings.smtp_password)
            server.send_message(message)
            logger.info(f"Correo enviado exitosamente a {recipient_email}")
    except smtplib.SMTPAuthenticationError:
        logger.error(f"Error de autenticación SMTP para {settings.smtp_user}. Verifique la contraseña de aplicación.")
    except Exception as e:
        logger.error(f"Error al enviar correo a {recipient_email}: {str(e)}")

def send_password_reset_email(email: str, token: str):
    """
    Envía el enlace de recuperación de contraseña al usuario con la identidad visual de la finca.
    """
    reset_link = f"{settings.frontend_url}/reset-password?token={token}"
    
    # Colores: 
    # Marine Green: #636b3f | Deep Green: #2b361c | Barium Yellow: #fefae3 
    # Sepia E37: #d4a369 | Leather: #b17036
    
    subject = "Recuperación de Contraseña - STGC Tierra Fértil"
    body_html = f"""
    <html>
        <body style="font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; line-height: 1.6; background-color: #fefae3; padding: 20px; color: #2b361c;">
            <div style="max-width: 600px; margin: auto; background-color: white; border: 2px solid #636b3f; padding: 40px; border-radius: 15px; box-shadow: 0 4px 15px rgba(0,0,0,0.1);">
                <div style="text-align: center; margin-bottom: 30px;">
                    <h1 style="color: #2b361c; margin: 0; font-size: 24px; border-bottom: 2px solid #d4a369; display: inline-block; padding-bottom: 10px;">
                        STGC Tierra Fértil
                    </h1>
                </div>
                
                <h2 style="color: #636b3f;">Restablecer Contraseña</h2>
                <p>Hola,</p>
                <p>Hemos recibido una solicitud para restablecer tu contraseña en el <strong>Sistema de Trazabilidad y Gestión de Café</strong>.</p>
                <p>Para continuar con el proceso, haz clic en el botón de abajo. Ten en cuenta que este enlace caducará en <strong>10 minutos</strong>:</p>
                
                <div style="text-align: center; margin: 40px 0;">
                    <a href="{reset_link}" 
                       style="background-color: #636b3f; color: #fefae3; padding: 15px 30px; text-decoration: none; border-radius: 8px; font-weight: bold; font-size: 16px; border: 1px solid #2b361c; display: inline-block;">
                       CREAR NUEVA CONTRASEÑA
                    </a>
                </div>
                
                <p style="font-size: 0.9em; color: #b17036;">
                    <strong>¿No solicitaste este cambio?</strong> Puedes ignorar este correo con total seguridad. Tu contraseña actual no cambiará.
                </p>
                
                <hr style="border: 0; border-top: 1px solid #d4a369; margin: 30px 0;">
                
                <p style="font-size: 0.8em; color: #2b361c; text-align: center; opacity: 0.8;">
                    Finca Tierra Fértil - Sistema de Gestión<br>
                    Este es un correo automático, por favor no responda.
                </p>
            </div>
        </body>
    </html>
    """
    send_email(email, subject, body_html)
