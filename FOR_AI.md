# Emotion Engine 2.0.0 — AI Agent Reference

> Rust-core VAD emotion engine. Plutchik 8 emotions. Memory coupling. MCP-native. OCEAN personality.

## INSTALL

```bash
# pip from git (recommended)
pip install git+https://github.com/Michaol/emotion_system.git

# or local build
cd openclaw-emotion-engine && pip install maturin && maturin develop --release
```

Verify: `emotion-mcp --help`

## MCP CONFIG

### Single agent
```json
{
  "mcpServers": {
    "emotion": {
      "command": "emotion-mcp",
      "env": { "EMOTION_AGENT_ID": "<agent_id>", "EMOTION_STATE_DIR": "./state" }
    }
  }
}
```

### Multi-agent
```json
{
  "mcpServers": {
    "emotion-alice": { "command": "emotion-mcp", "env": { "EMOTION_AGENT_ID": "Alice", "EMOTION_STATE_DIR": "./state" } },
    "emotion-bob": { "command": "emotion-mcp", "env": { "EMOTION_AGENT_ID": "Bob", "EMOTION_STATE_DIR": "./state" } }
  }
}
```

### Remote SSE
```bash
fastmcp run emotion_engine.mcp_server:mcp --transport sse --host 0.0.0.0 --port 8080
```
```json
{ "mcpServers": { "emotion": { "url": "http://<host>:8080/sse" } } }
```

## TOOLS

All accept optional `agent_id` (default: env `EMOTION_AGENT_ID`).

| tool | params | effect |
|---|---|---|
| `apply_emotion_event` | `event_name: str`, `intensity: 0-5` | Event → VAD delta via config rules. Updates Plutchik emotion if name matches. |
| `modify_emotion_dimension` | `dimension: v\|a\|d`, `delta: float` | Direct VAD nudge |
| `reset_emotion_state` | — | Reset VAD + Plutchik to baseline |
| `reflect_emotion` | — | Analyze VAD deviation → text reflection |
| `dream_emotion` | — | Stochastic micro-fluctuation |
| `evolve_personality` | — | Long-term OCEAN drift (needs ≥10 events) |

## RESOURCES

| uri | returns |
|---|---|
| `emotion://state/{agent_id}` | VAD + Plutchik label/confidence + memories (JSON) |
| `emotion://prompt/{agent_id}` | XML with `<plutchik>`, `<memories>`, `<dimensions>`, `<tone>` |
| `emotion://personality/{agent_id}` | OCEAN values (JSON) |
| `emotion://reflect/{agent_id}` | Latest reflection text |

## EMOTION MODEL

**VAD** (continuous): Valence [-1,1], Arousal [0,1], Dominance [0,1]

**Plutchik** (discrete, auto-derived from VAD via KNN): joy, trust, fear, surprise, sadness, disgust, anger, anticipation. Each has an opposite (joy↔sadness, trust↔disgust, fear↔anger, surprise↔anticipation). Updating one auto-nudges its opposite.

**Memory**: Events stored with VAD tag. Decay via power law. Frequent recall strengthens memory. Low-retention entries auto-GC'd.

## PERSONALITY (OCEAN)

`openness`, `conscientiousness`, `extraversion`, `agreeableness`, `neuroticism` — float 0.0~1.0

```
config/<agent_id>.json   → template (used only if no state file)
state/<agent_id>.json    → live state (auto-generated, editable)
```

Example: `config/alice.json`
```json
{"openness":0.8,"conscientiousness":0.5,"extraversion":0.9,"agreeableness":0.7,"neuroticism":0.3}
```

## ENV VARS

| var | default | purpose |
|---|---|---|
| `EMOTION_AGENT_ID` | `"default"` | Agent identity |
| `EMOTION_STATE_DIR` | `./state` | State directory |
| `EMOTION_CONFIG_DIR` | `./config` | Personality templates |

## DASHBOARD

```bash
emotion-bridge   # → http://localhost:8000
```
