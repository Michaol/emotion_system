# BUG: OCEAN Personality Not Loaded from Config on Re-init [FINISHED]

**Severity:** Medium (behavioral drift)
**Status:** ✅ FIXED
**Fixed at:** 2026-03-21
**Component:** `emotion-core` / `Engine::new()` (Rust) → `EngineWrapper.__init__` (Python)
**Date:** 2026-03-21
**Reported by:** Asuna (operator)

## Summary

The OCEAN personality profile from `config/<agent_id>.json` is **not applied** when the engine is re-initialized with an existing state file. Only the first-ever init (when state file doesn't exist) loads personality from config. On subsequent inits, the engine uses whatever personality was previously persisted in the state file — which defaults to all-0.5 if the state was created before personality was ever set.

## Root Cause

In `core/src/engine.rs`, `Engine::new()` has explicit conditional logic (marked `BUG-L2` in source):

```rust
// BUG-L2: 仅在首次初始化（state 文件不存在）时从 agent config 加载 OCEAN 人格
// 如果 state 文件已存在，说明人格可能已被 drift/set_personality 修改过，应尊重持久化值
if !state_exists {
    if let Some(aid) = agent_id {
        // ... load from config/{aid}.json ...
    }
}
```

**The intended behavior:** Avoid overwriting evolved/drifted personality with config defaults.

**The actual bug:** When the state file is first created, `EmotionState::default()` sets personality to `OceanProfile::default()` (all 0.5). Then the agent config IS loaded correctly (state didn't exist). BUT — if the state file was created by a code path that didn't pass `agent_id` or used a different config, the state persists with 0.5 forever, and re-inits with the correct `agent_id` won't fix it.

More critically: `get_engine()` in `engine.py` passes `agent_id` to `_RustEngine`, but the singleton pattern means if the engine was first created without proper config (e.g., during testing or a previous bug), the 0.5 personality is baked into the state file permanently.

## Reproduction

1. First init: state file doesn't exist → personality loaded from `config/asuna.json` → saved with O(0.70), C(0.95), E(0.75), A(0.80), N(0.25) ✓
2. Observe state file has correct OCEAN values
3. Delete state file (or use fresh agent_id)
4. Init again without explicit `agent_id` → state created with 0.5 defaults
5. Init again WITH `agent_id='asuna'` → state file exists → **config is NOT re-read** → personality stays at 0.5 ✗

## Impact

- Decay rates are wrong (calculated from neutral personality instead of agent-specific)
- Baseline VAD offsets are wrong
- The agent behaves "generically" instead of reflecting its configured personality

## Observed State

```
# Before fix (state file had 0.5 defaults):
personality: {O:0.5, C:0.5, E:0.5, A:0.5, N:0.5}

# Config (config/asuna.json) has correct values:
personality: {O:0.70, C:0.95, E:0.75, A:0.80, N:0.25}

# After manual set_personality + save:
personality: {O:0.70, C:0.95, E:0.75, A:0.80, N:0.25} ✓
```

## Proposed Fix

### Option A: Config-Always-Wins on Init (Simple)

```rust
// In Engine::new(), after loading state:
if let Some(aid) = agent_id {
    let agent_config_path = config_dir.join(format!("{aid}.json"));
    if agent_config_path.exists() {
        // Always load OCEAN from config, regardless of state_exists
        if let Ok(content) = std::fs::read_to_string(&agent_config_path) {
            if let Ok(agent_cfg) = serde_json::from_str::<AgentConfig>(&content) {
                if let Some(personality) = agent_cfg.personality {
                    state.baseline = compute_baseline(&personality);
                    state.decay_rates = DecayRates::from_personality(&personality);
                    state.personality = personality;
                }
            }
        }
    }
}
```

**Trade-off:** Loses any evolved/drifted personality on restart. Simpler, more predictable.

### Option B: Config as Fallback (State Takes Priority, Config Fills Gaps)

```rust
// Only override if personality is still at defaults (all 0.5)
if is_default_personality(&state.personality) {
    if let Some(aid) = agent_id {
        // ... load from config ...
    }
}
```

**Trade-off:** Preserves evolved personality while fixing the 0.5-default problem.

### Option C: Merge/Versioned State

Add a `personality_version` or `initialized_from_config` flag to state. Only load from config if the flag is false or version mismatches.

## Recommendation

**Option A** is recommended. Personality drift via `evolve()` is minimal (±0.005 per call) and the config file represents the authoritative character definition. If drift is desired, it should be explicitly enabled via a config flag.

## Workaround

After `get_engine()`, call `set_personality()` for each trait from the config, then `save()`:

```python
eng = ee.get_engine(agent_id='asuna', config_dir='config', state_dir='state')
eng.set_personality('openness', 0.70)
eng.set_personality('conscientiousness', 0.95)
eng.set_personality('extraversion', 0.75)
eng.set_personality('agreeableness', 0.80)
eng.set_personality('neuroticism', 0.25)
eng.save()
```

This is already applied in the periodic decay scripts as a band-aid fix.

## Files

- Bug location: `core/src/engine.rs` — `Engine::new()` (lines ~83-98)
- Config: `config/asuna.json`
- State: `state/asuna_state.json`
- Python wrapper: `python/emotion_engine/engine.py` — `EngineWrapper.__init__`

## 修复记录

**Fixed at:** 2026-03-21
**方案:** Option A — Config-Always-Wins on Init

### 修改

`core/src/engine.rs:118-150` — 移除 `if !state_exists` 守卫，始终从 config 加载 OCEAN 人格。

### 验证

- 56 个 Rust 测试全部通过
- 无论 state 文件是否存在，config 中的 OCEAN 值都会被应用
