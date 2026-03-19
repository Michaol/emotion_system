"""
emotion_engine — VAD-based emotional simulation for AI agents

Public API:
    EngineWrapper — Thread-safe engine wrapper with Reflect/Dream/Evolve
    get_engine   — Singleton factory (by agent_id)
"""

from .engine import EngineWrapper, get_engine

__all__ = ["EngineWrapper", "get_engine"]
