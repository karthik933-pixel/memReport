import json
import os
import shutil
import requests
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from google import genai
from google.genai import types
from pydantic import BaseModel

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["POST", "OPTIONS"],
    allow_headers=["*"],
)

RUST_BASE_URL = "http://127.0.0.1:8080"
DRIVES = ["C:\\", "D:\\", "E:\\"]


class AskRequest(BaseModel):
    question: str


def get_windows_drive_stats() -> list[dict]:
    stats = []
    gb = 1024 ** 3
    for drive in DRIVES:
        if os.path.exists(drive):
            total, used, free = shutil.disk_usage(drive)
            stats.append(
                {
                    "drive": drive,
                    "total_gb": round(total / gb, 2),
                    "used_gb": round(used / gb, 2),
                    "free_gb": round(free / gb, 2),
                    "used_percent": round((used / total) * 100, 2) if total > 0 else 0.0,
                }
            )
    return stats


def get_rust_telemetry() -> dict:
    telemetry = {}
    for endpoint in ("/api/storage", "/api/cpu", "/api/stats"):
        try:
            res = requests.get(f"{RUST_BASE_URL}{endpoint}", timeout=1.5)
            if res.ok:
                telemetry[endpoint] = res.json()
        except Exception:
            continue
    return telemetry


@app.post("/api/ask")
def ask_gemini(req: AskRequest):
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        raise HTTPException(
            status_code=500,
            detail="GEMINI_API_KEY environment variable is not set.",
        )

    storage_context = {
        "drives_summary": get_windows_drive_stats(),
        "rust_server_telemetry": get_rust_telemetry(),
    }

    system_prompt = (
        "You are an AI assistant embedded in a Windows System & Storage Monitor dashboard. "
        "Answer the user's question about their disk storage, drive capacities, free/used space, "
        "or largest files concisely and accurately using the live telemetry JSON below.\n\n"
        f"LIVE STORAGE TELEMETRY:\n{json.dumps(storage_context, indent=2)}"
    )

    try:
        client = genai.Client(api_key=api_key)
        response = client.models.generate_content(
            model="gemini-2.5-flash",
            contents=req.question,
            config=types.GenerateContentConfig(
                system_instruction=system_prompt,
            ),
        )
        return {"answer": response.text}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Gemini API error: {e}")