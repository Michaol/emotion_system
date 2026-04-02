use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{BehaviorConfig, ConfigError, EventConfig};
use crate::decay::{ms_to_hours, split_decay};
use crate::decay_schedule::DecaySchedule;
use crate::memory::EmotionalMemory;
use crate::personality::{compute_baseline, is_default_personality, DecayRates, OceanProfile};
use crate::plutchik::{self, PlutchikResult, PlutchikState};
use crate::rumination::{
    add_rumination, advance_ruminations, should_ruminate, RuminationEntry, RUMINATION_THRESHOLD,
};
use crate::vad::VadState;

/// 引擎返回的情绪快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionSnapshot {
    pub v: f32,
    pub a: f32,
    pub d: f32,
    pub dominant_emotion: String,
    pub tone: String,
    pub active_ruminations: usize,
    pub personality: OceanProfile,
    pub plutchik: PlutchikResult,
}

/// 刺激历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StimulusRecord {
    pub event_name: String,
    pub delta: VadState,
    pub timestamp_ms: i64,
    pub triggered_rumination: bool,
}

/// 存储到磁盘的完整状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionState {
    pub current: VadState,
    pub baseline: VadState,
    pub personality: OceanProfile,
    pub decay_rates: DecayRates,
    pub ruminations: Vec<RuminationEntry>,
    pub stimuli_history: VecDeque<StimulusRecord>,
    pub last_updated_ms: i64,
    #[serde(default)]
    pub plutchik: PlutchikState,
    #[serde(default)]
    pub memories: Vec<EmotionalMemory>,
    #[serde(default)]
    pub decay_schedule: DecaySchedule,
}

impl Default for EmotionState {
    fn default() -> Self {
        let personality = OceanProfile::default();
        let baseline = compute_baseline(&personality);
        let decay_rates = DecayRates::from_personality(&personality);
        Self {
            current: baseline.clone(),
            baseline,
            personality,
            decay_rates,
            ruminations: Vec::new(),
            stimuli_history: VecDeque::new(),
            last_updated_ms: now_ms(),
            plutchik: PlutchikState::default(),
            memories: Vec::new(),
            decay_schedule: DecaySchedule::default(),
        }
    }
}

/// Agent 专属配置（从 config/<agent_id>.json 加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub personality: Option<OceanProfile>,
}

/// 核心引擎
pub struct Engine {
    pub state: EmotionState,
    pub event_config: EventConfig,
    pub behavior_config: BehaviorConfig,
    pub state_path: String,
    pub dirty: bool,
}

impl Engine {
    /// 创建引擎：加载配置和状态
    ///
    /// - `config_path`: 配置目录（含 default_events.json, default_behavior.json）
    /// - `state_path`: 状态文件路径
    /// - `agent_id`: 可选 Agent 标识，用于从 `config/<agent_id>.json` 加载 OCEAN 人格
    pub fn new(
        config_path: &str,
        state_path: &str,
        agent_id: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let config_dir = Path::new(config_path);
        let event_config = EventConfig::load(&config_dir.join("default_events.json"))?;
        let behavior_config = BehaviorConfig::load(&config_dir.join("default_behavior.json"))?;

        // BUG-L4: 自动创建 state 文件的父目录
        if let Some(parent) = Path::new(state_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let state_exists = Path::new(state_path).exists();
        let mut state = if state_exists {
            let content = std::fs::read_to_string(state_path)?;
            serde_json::from_str(&content)?
        } else {
            EmotionState::default()
        };

        // Option B: 仅当 state 中的人格仍为默认值 (0.5) 时从 config 加载
        // - state 为 0.5 → 从未加载过 config 或 bug 导致 → 从 config 加载
        // - state 已漂移 → 保留漂移值，不覆盖
        // - state 文件不存在 → 已在上面用 default 创建，人格为 0.5 → 从 config 加载
        if is_default_personality(&state.personality) {
            if let Some(aid) = agent_id {
                let agent_config_path = config_dir.join(format!("{aid}.json"));
                if agent_config_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&agent_config_path) {
                        match serde_json::from_str::<AgentConfig>(&content) {
                            Ok(agent_cfg) => {
                                if let Some(personality) = agent_cfg.personality {
                                    state.baseline = compute_baseline(&personality);
                                    state.decay_rates = DecayRates::from_personality(&personality);
                                    state.personality = personality;
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "[emotion-engine] Error: failed to parse agent config '{}': {}",
                                    agent_config_path.display(),
                                    e
                                );
                            }
                        }
                    }
                } else if aid != "default" {
                    eprintln!(
                        "[emotion-engine] Debug: agent config file not found: {}",
                        agent_config_path.display()
                    );
                }
            }
        }

        Ok(Self {
            state,
            event_config,
            behavior_config,
            state_path: state_path.to_string(),
            dirty: false,
        })
    }

    /// 应用按需衰减 (含昼夜分段)
    fn apply_decay_to_current(&mut self) {
        let now = now_ms();
        let last = self.state.last_updated_ms;
        let delta_hours = ms_to_hours(now - last);
        if delta_hours <= 0.0 {
            return;
        }

        self.state.current.v = split_decay(
            self.state.current.v,
            self.state.baseline.v,
            self.state.decay_rates.v_rate,
            last,
            now,
            "v",
            &self.state.decay_schedule,
        );
        self.state.current.a = split_decay(
            self.state.current.a,
            self.state.baseline.a,
            self.state.decay_rates.a_rate,
            last,
            now,
            "a",
            &self.state.decay_schedule,
        );
        self.state.current.d = split_decay(
            self.state.current.d,
            self.state.baseline.d,
            self.state.decay_rates.d_rate,
            last,
            now,
            "d",
            &self.state.decay_schedule,
        );

        // 随着时间推进反刍（余波累加）
        // 由于此处有 delta_hours > 0 的守卫，保证了在同一毫秒内多次调用 get_state 是幂等的
        advance_ruminations(&mut self.state.current, &mut self.state.ruminations);

        // 更新情感记忆衰减
        if !self.state.memories.is_empty() {
            for mem in &mut self.state.memories {
                let hours_since = crate::decay::ms_to_hours(now - mem.timestamp_ms);
                mem.update_retention(hours_since, 0.5, 1.0);
            }
            // 回收低留存记忆 (retention < 0.05)
            self.state.memories.retain(|m| !m.should_gc(0.05));
        }

        self.state.last_updated_ms = now;
        self.dirty = true; // 衰减改变了状态，标记脏以确保持久化
    }

    /// 构造快照
    fn snapshot(&self) -> EmotionSnapshot {
        let (tone, _speed, _desc) = self.behavior_config.resolve(
            self.state.current.v,
            self.state.current.a,
            self.state.current.d,
        );

        let dominant = self.find_dominant_emotion();
        let plutchik_result = plutchik::classify_plutchik(&self.state.current, 3);

        EmotionSnapshot {
            v: self.state.current.v,
            a: self.state.current.a,
            d: self.state.current.d,
            dominant_emotion: dominant,
            tone: tone.to_string(),
            active_ruminations: self.state.ruminations.len(),
            personality: self.state.personality.clone(),
            plutchik: plutchik_result,
        }
    }

    /// 查找与当前状态最接近的情绪标签 (使用 Plutchik 分类)
    fn find_dominant_emotion(&self) -> String {
        let plutchik_result = plutchik::classify_plutchik(&self.state.current, 3);
        plutchik_result.label
    }

    /// 应用事件 — 未知事件不再崩溃，返回当前快照并打印警告
    pub fn apply_event(
        &mut self,
        name: &str,
        intensity: f32,
    ) -> Result<EmotionSnapshot, ConfigError> {
        self.apply_decay_to_current();

        // BUG-L1: 未知事件容错 — 返回零 delta 快照 + 警告
        let delta = match self.event_config.get_delta(name) {
            Ok(d) => d.clone(),
            Err(_) => {
                eprintln!(
                    "[emotion-engine] Warning: unknown event '{}', ignored",
                    name
                );
                return Ok(self.snapshot());
            }
        };
        let scaled = VadState::new(
            delta.delta_v * intensity,
            delta.delta_a * intensity,
            delta.delta_d * intensity,
        );

        // 在 apply_delta 之前捕获当前 VAD (用于 EmotionalMemory.vad_at_event)
        let vad_before = self.state.current.clone();
        let now = now_ms();

        self.state.current.apply_delta(scaled.v, scaled.a, scaled.d);

        let triggered = should_ruminate(&scaled, RUMINATION_THRESHOLD);
        if triggered {
            add_rumination(
                &mut self.state.ruminations,
                RuminationEntry::new(name.to_string(), scaled.clone()),
            );
        }

        // 更新 Plutchik 情绪状态 (如果事件名匹配已知情绪)
        if self.state.plutchik.get(name).is_some() {
            self.state.plutchik.update_with_opposite(
                name,
                intensity * 0.5,
                plutchik::DEFAULT_OPPOSITE_FACTOR,
            );
        }

        // 记录情感记忆 (vad_at_event 使用 delta 之前的快照)
        let mem = EmotionalMemory::new(name.to_string(), vad_before, scaled.clone(), now);
        self.state.memories.push(mem);
        if self.state.memories.len() > 100 {
            self.state
                .memories
                .drain(0..self.state.memories.len() - 100);
        }

        // 记录刺激历史 (保留最近 20 条)
        self.state.stimuli_history.push_back(StimulusRecord {
            event_name: name.to_string(),
            delta: scaled,
            timestamp_ms: now,
            triggered_rumination: triggered,
        });
        if self.state.stimuli_history.len() > 20 {
            self.state.stimuli_history.pop_front();
        }

        self.state.last_updated_ms = now;
        self.dirty = true;
        Ok(self.snapshot())
    }

    /// 检查事件名是否存在于配置中
    #[must_use]
    pub fn has_event(&self, name: &str) -> bool {
        self.event_config.events.contains_key(name)
    }

    /// 获取当前状态 (只读，应用衰减但不标脏)
    pub fn get_state(&mut self) -> EmotionSnapshot {
        self.apply_decay_to_current();
        self.snapshot()
    }

    /// 直接修改单个维度
    pub fn modify(&mut self, dimension: &str, delta: f32) -> Result<EmotionSnapshot, ConfigError> {
        self.apply_decay_to_current();
        match dimension {
            "v" | "valence" => self.state.current.apply_delta(delta, 0.0, 0.0),
            "a" | "arousal" => self.state.current.apply_delta(0.0, delta, 0.0),
            "d" | "dominance" => self.state.current.apply_delta(0.0, 0.0, delta),
            _ => {
                return Err(ConfigError::UnknownEvent(format!(
                    "Unknown dimension: {dimension}"
                )))
            }
        }
        self.state.last_updated_ms = now_ms();
        self.dirty = true;
        Ok(self.snapshot())
    }

    /// 重置至 baseline
    pub fn reset(&mut self, dimensions: Option<&[String]>) -> EmotionSnapshot {
        match dimensions {
            Some(dims) => {
                for dim in dims {
                    match dim.as_str() {
                        "v" | "valence" => self.state.current.v = self.state.baseline.v,
                        "a" | "arousal" => self.state.current.a = self.state.baseline.a,
                        "d" | "dominance" => self.state.current.d = self.state.baseline.d,
                        _ => {}
                    }
                }
            }
            None => {
                self.state.current = self.state.baseline.clone();
                self.state.ruminations.clear();
                self.state.plutchik.reset();
            }
        }
        self.state.last_updated_ms = now_ms();
        self.dirty = true;
        self.snapshot()
    }

    /// 设置人格特质并重算 baseline/decay_rates
    pub fn set_personality(&mut self, trait_name: &str, value: f32) -> Result<(), String> {
        self.state.personality.set_trait(trait_name, value)?;
        self.state.baseline = compute_baseline(&self.state.personality);
        self.state.decay_rates = DecayRates::from_personality(&self.state.personality);
        self.dirty = true;
        Ok(())
    }

    /// 获取人格配置
    #[must_use]
    pub fn get_personality(&self) -> &OceanProfile {
        &self.state.personality
    }

    /// 检索与当前情绪相关的记忆
    #[must_use]
    pub fn get_memories(&self, top_k: usize) -> Vec<&EmotionalMemory> {
        crate::memory::retrieve_memories(&self.state.memories, &self.state.current, top_k)
    }
}

/// 获取当前 Unix 时间戳 (毫秒)
///
/// 如果系统时间异常 (早于 UNIX_EPOCH)，返回 0 并打印警告。
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|e| {
            eprintln!("[emotion-engine] Warning: system time before UNIX_EPOCH: {e}");
            0
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 返回 TempDir 句柄以保持目录存活
    fn create_test_env() -> (TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let events = r#"{
            "version": "1.0",
            "events": {
                "joy": { "delta_v": 0.4, "delta_a": 0.2, "delta_d": 0.1 },
                "anger": { "delta_v": -0.5, "delta_a": 0.6, "delta_d": 0.3 }
            }
        }"#;
        let behaviors = r#"{
            "version": "1.0",
            "behaviors": [
                {
                    "condition": { "v_min": 0.3 },
                    "tone": "cheerful",
                    "speed": "moderate",
                    "description": "happy"
                }
            ],
            "default": {
                "tone": "neutral",
                "speed": "moderate",
                "description": "stable"
            }
        }"#;

        std::fs::write(dir.path().join("default_events.json"), events).unwrap();
        std::fs::write(dir.path().join("default_behavior.json"), behaviors).unwrap();

        let state_path = dir.path().join("state.json").to_string_lossy().to_string();
        (dir, state_path)
    }

    #[test]
    fn test_engine_creation() {
        let (dir, state_path) = create_test_env();
        let engine = Engine::new(dir.path().to_str().unwrap(), &state_path, None);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_apply_event() {
        let (dir, state_path) = create_test_env();
        let mut engine = Engine::new(dir.path().to_str().unwrap(), &state_path, None).unwrap();

        let snap = engine.apply_event("joy", 1.0).unwrap();
        assert!(snap.v > 0.0);
        assert!(engine.dirty);
    }

    #[test]
    fn test_unknown_event_is_tolerated() {
        let (dir, state_path) = create_test_env();
        let mut engine = Engine::new(dir.path().to_str().unwrap(), &state_path, None).unwrap();
        // BUG-L1: 未知事件不再报错，返回当前快照
        let result = engine.apply_event("nonexistent", 1.0);
        assert!(result.is_ok());
        let snap = result.unwrap();
        // VAD 应无变化 (接近中性默认)
        assert!(snap.v.abs() < 0.1);
    }

    #[test]
    fn test_has_event() {
        let (dir, state_path) = create_test_env();
        let engine = Engine::new(dir.path().to_str().unwrap(), &state_path, None).unwrap();
        assert!(engine.has_event("joy"));
        assert!(engine.has_event("anger"));
        assert!(!engine.has_event("nonexistent"));
    }

    #[test]
    fn test_engine_with_agent_config() {
        let (dir, state_path) = create_test_env();
        // 写入 agent 配置
        let agent_json = r#"{
            "personality": {
                "openness": 0.75,
                "conscientiousness": 0.95,
                "extraversion": 0.80,
                "agreeableness": 0.70,
                "neuroticism": 0.35
            }
        }"#;
        std::fs::write(dir.path().join("asuna.json"), agent_json).unwrap();

        let engine = Engine::new(dir.path().to_str().unwrap(), &state_path, Some("asuna")).unwrap();
        let p = engine.get_personality();
        assert!((p.openness - 0.75).abs() < 0.01);
        assert!((p.conscientiousness - 0.95).abs() < 0.01);
        assert!((p.neuroticism - 0.35).abs() < 0.01);
    }

    #[test]
    fn test_option_b_drifted_personality_preserved() {
        let (dir, state_path) = create_test_env();
        let agent_json = r#"{
            "personality": {
                "openness": 0.70,
                "conscientiousness": 0.95,
                "extraversion": 0.80,
                "agreeableness": 0.75,
                "neuroticism": 0.25
            }
        }"#;
        std::fs::write(dir.path().join("asuna.json"), agent_json).unwrap();

        // 首次创建：从 config 加载
        let mut engine =
            Engine::new(dir.path().to_str().unwrap(), &state_path, Some("asuna")).unwrap();
        assert!((engine.get_personality().openness - 0.70).abs() < 0.01);

        // 模拟 drift：修改 personality 并保存
        engine.state.personality.openness = 0.705; // 微小漂移
        crate::persistence::save_state(&mut engine).unwrap();

        // 重启：state 中 personality 不是 0.5 → Option B 应保留漂移值
        let engine2 =
            Engine::new(dir.path().to_str().unwrap(), &state_path, Some("asuna")).unwrap();
        assert!(
            (engine2.get_personality().openness - 0.705).abs() < 0.01,
            "Drifted openness should be preserved, got {}",
            engine2.get_personality().openness
        );
    }

    #[test]
    fn test_option_b_default_personality_overridden() {
        let (dir, state_path) = create_test_env();
        let agent_json = r#"{
            "personality": {
                "openness": 0.85,
                "conscientiousness": 0.60,
                "extraversion": 0.90,
                "agreeableness": 0.75,
                "neuroticism": 0.25
            }
        }"#;
        std::fs::write(dir.path().join("asuna.json"), agent_json).unwrap();

        // 创建 state 文件但 personality 为 0.5（模拟 bug 场景）
        let default_state = EmotionState::default();
        let json = serde_json::to_string(&default_state).unwrap();
        std::fs::write(&state_path, json).unwrap();

        // 重启：state 中 personality 为 0.5 → Option B 应从 config 加载
        let engine = Engine::new(dir.path().to_str().unwrap(), &state_path, Some("asuna")).unwrap();
        assert!(
            (engine.get_personality().openness - 0.85).abs() < 0.01,
            "Default 0.5 should be overridden by config, got {}",
            engine.get_personality().openness
        );
    }
}
