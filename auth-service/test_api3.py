import asyncio
from app.main import app
from fastapi.testclient import TestClient
from app.database import db

async def test():
    await db.connect()
    # Ensure user exists
    user = await db.user.find_first()
    if user:
        client = TestClient(app)
        # Use the actual user's email
        response = client.post("/api/auth/password-recovery", json={"email": user.email})
        print("Response:", response.status_code, response.json())
    else:
        print("No user found in DB")
    await db.disconnect()

asyncio.run(test())
