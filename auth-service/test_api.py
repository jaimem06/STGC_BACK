import asyncio
from app.main import app
from fastapi.testclient import TestClient

client = TestClient(app)

response = client.post("/api/auth/password-recovery", json={"email": "admin@stgc.local"})
print("Response:", response.status_code, response.json())
