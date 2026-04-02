"""
Emotion Engine Bridge Server — FastAPI Bridge for Web Dashboard

To run from package (standard):
    python -m emotion_engine.bridge
To run from script (dev):
    python bridge.py
"""

import os
import sys
from typing import List, Optional

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel
from fastapi.responses import FileResponse

try:
    from .engine import get_engine
except (ImportError, ValueError):
    from engine import get_engine

app = FastAPI(title="Emotion Engine Bridge")

# Enable CORS for the dashboard
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:8000", "http://127.0.0.1:8000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Static files (the dashboard)
_BASE_DIR = os.path.dirname(__file__)
DASHBOARD_DIR = os.path.join(_BASE_DIR, "dashboard")
app.mount("/static", StaticFiles(directory=DASHBOARD_DIR), name="static")


@app.get("/", include_in_schema=False)
async def read_index():
    return FileResponse(os.path.join(DASHBOARD_DIR, "index.html"))


# Shared Instance
engine = get_engine(agent_id="dashboard_test")

# ── Models ──────────────────────────────


class EventRequest(BaseModel):
    name: str
    intensity: float = 1.0


class ResetRequest(BaseModel):
    dimensions: Optional[List[str]] = None


class ModifyRequest(BaseModel):
    dimension: str
    delta: float


# ── Endpoints ───────────────────────────


@app.get("/state")
def get_state():
    """Returns combined state for the dashboard"""
    return {
        "snapshot": engine.get_state(),
        "personality": engine.get_personality(),
        "prompt": engine.format_prompt(),
        "reflect": engine.last_reflect,
    }


@app.post("/apply", responses={400: {"description": "Invalid event name"}})
def apply_event(req: EventRequest):
    try:
        engine.apply_event(req.name, req.intensity)
        return get_state()
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@app.post("/modify", responses={400: {"description": "Invalid dimension"}})
def modify(req: ModifyRequest):
    try:
        engine.modify(req.dimension, req.delta)
        return get_state()
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@app.post("/reset")
def reset(req: Optional[ResetRequest] = None):
    dims = req.dimensions if req else None
    engine.reset(dims)
    return get_state()


@app.post("/reflect")
def reflect():
    """Force a self-reflection"""
    text = engine.reflect()
    return {"reflect": text, **get_state()}


@app.post("/dream")
def dream():
    """Trigger a dream sequence"""
    result = engine.dream()
    return {"dream": result, **get_state()}


@app.post("/evolve")
def evolve():
    """Trigger personality evolution"""
    drift = engine.evolve()
    return {"drift": drift, **get_state()}


@app.get("/agents")
def get_agents():
    return engine.get_other_agents()


def main():
    import uvicorn

    print("\n🚀 Emotion Engine Bridge Server running at http://localhost:8000")
    print("👉 Open dashboard/index.html in your browser!\n")
    # 使用字符串形式启动以支持更健壮的重载
    uvicorn.run("emotion_engine.bridge:app", host="0.0.0.0", port=8000, reload=True)


if __name__ == "__main__":
    main()
