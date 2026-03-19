"""
Emotion Engine MCP Server — Exposing Emotions as Resources

Provides:
  - Resource: emotion://state/{agent_id}
  - Resource: emotion://personality/{agent_id}
  - Resource: emotion://prompt/{agent_id}
  - Resource: emotion://reflect/{agent_id}
  - Tool: apply_emotion_event
  - Tool: modify_emotion_dimension
  - Tool: reset_emotion_state
  - Tool: reflect_emotion
  - Tool: dream_emotion
  - Tool: evolve_personality

To run:
    pip install fastmcp
    python mcp_server.py
"""
import json
from typing import Annotated, Any
from pydantic import Field
from fastmcp import FastMCP
try:
    from .engine import get_engine
except (ImportError, ValueError):
    from engine import get_engine

# ── Initialization ────────────────────────

mcp = FastMCP("Emotion-Engine")

# ── Resources ─────────────────────────────

@mcp.resource("emotion://state/{agent_id}")
def get_emotion_state(agent_id: str) -> str:
    """Get the current VAD state of an agent as a JSON string"""
    engine = get_engine(agent_id=agent_id)
    return json.dumps(engine.get_state(), indent=2)

@mcp.resource("emotion://personality/{agent_id}")
def get_personality(agent_id: str) -> str:
    """Get the personality traits of an agent as a JSON string"""
    engine = get_engine(agent_id=agent_id)
    return json.dumps(engine.get_personality(), indent=2)

@mcp.resource("emotion://prompt/{agent_id}")
def get_emotion_prompt(agent_id: str) -> str:
    """Get the formatted <emotion_state> XML prompt for context injection"""
    engine = get_engine(agent_id=agent_id)
    return engine.format_prompt()

@mcp.resource("emotion://reflect/{agent_id}")
def get_reflect(agent_id: str) -> str:
    """Get the latest self-reflection text for an agent"""
    engine = get_engine(agent_id=agent_id)
    return engine.last_reflect or "(no reflection yet)"

# ── Tools ─────────────────────────────────

@mcp.tool()
def apply_emotion_event(
    agent_id: Annotated[str, Field(description="The unique identifier for the agent (default: 'default')")],
    event_name: Annotated[str, Field(description="The name of the emotional event to apply (e.g., 'joy', 'anger', 'stress', 'insult')")],
    intensity: Annotated[float, Field(description="The multiplier for the event's impact on VAD dimensions", ge=0.0, le=5.0)] = 1.0,
) -> str:
    """Apply an emotional stimulus/event to an agent"""
    engine = get_engine(agent_id=agent_id)
    try:
        snap = engine.apply_event(event_name, intensity)
        return f"Event '{event_name}' applied to {agent_id}. Tone: {snap['tone']}"
    except ValueError as e:
        return f"Error: {e}"

@mcp.tool()
def modify_emotion_dimension(
    agent_id: Annotated[str, Field(description="The unique identifier for the agent")],
    dimension: Annotated[str, Field(description="The VAD dimension attribute to check: 'v', 'a', or 'd'")],
    delta: Annotated[float, Field(description="The numerical value to add/subtract from the current dimension state")]
) -> str:
    """Directly calibrate an emotional dimension (v, a, or d)"""
    engine = get_engine(agent_id=agent_id)
    try:
        snap = engine.modify(dimension, delta)
        return f"V={snap['v']:.2f}, A={snap['a']:.2f}, D={snap['d']:.2f}"
    except ValueError as e:
        return f"Error: {e}"

@mcp.tool()
def reset_emotion_state(agent_id: str) -> str:
    """Reset the agent's emotions to their baseline personality level"""
    engine = get_engine(agent_id=agent_id)
    engine.reset()
    return f"{agent_id} reset to baseline."

@mcp.tool()
def reflect_emotion(agent_id: str) -> str:
    """Force the agent to perform emotional self-reflection"""
    engine = get_engine(agent_id=agent_id)
    return engine.reflect()

@mcp.tool()
def dream_emotion(agent_id: str) -> str:
    """Trigger a dream sequence — random micro-VAD perturbation simulating subconscious processing"""
    engine = get_engine(agent_id=agent_id)
    result = engine.dream()
    return f"Dream theme: {result['theme']}. Mods: {result['modifications']}"

@mcp.tool()
def evolve_personality(agent_id: str) -> str:
    """Execute personality drift based on accumulated emotional history"""
    engine = get_engine(agent_id=agent_id)
    drift = engine.evolve()
    if not drift:
        return "Insufficient history (need ≥10 events). No drift applied."
    return f"Personality drifted: {drift}"

def main():
    """MCP entry point"""
    mcp.run()

if __name__ == "__main__":
    main()
