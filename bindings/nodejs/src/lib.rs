use napi::bindgen_prelude::*;
use napi_derive::napi;

use emotion_core::engine::Engine;
use emotion_core::multi_agent;
use emotion_core::persistence;
use emotion_core::prompt;

/// JS 可见的 EmotionSnapshot
#[napi(object)]
pub struct EmotionSnapshot {
    pub v: f64,
    pub a: f64,
    pub d: f64,
    pub dominant_emotion: String,
    pub tone: String,
    pub active_ruminations: i64,
    pub plutchik_label: String,
    pub plutchik_confidence: f64,
}

/// JS 可见的 OceanProfile
#[napi(object)]
pub struct OceanProfile {
    pub openness: f64,
    pub conscientiousness: f64,
    pub extraversion: f64,
    pub agreeableness: f64,
    pub neuroticism: f64,
}

/// JS 可见的 AgentSummary
#[napi(object)]
pub struct AgentSummary {
    pub agent_id: String,
    pub v: f64,
    pub a: f64,
    pub d: f64,
    pub last_updated_ms: i64,
}

fn to_js_snap(s: &emotion_core::engine::EmotionSnapshot) -> EmotionSnapshot {
    EmotionSnapshot {
        v: f64::from(s.v),
        a: f64::from(s.a),
        d: f64::from(s.d),
        dominant_emotion: s.dominant_emotion.clone(),
        tone: s.tone.clone(),
        active_ruminations: s.active_ruminations as i64,
        plutchik_label: s.plutchik.label.clone(),
        plutchik_confidence: f64::from(s.plutchik.confidence),
    }
}

/// EmotionEngine — Node.js 原生绑定
#[napi]
pub struct EmotionEngine {
    inner: Engine,
}

#[napi]
impl EmotionEngine {
    /// 构造函数
    #[napi(constructor)]
    pub fn new(config_path: String, state_path: String, agent_id: Option<String>) -> Result<Self> {
        let inner = Engine::new(&config_path, &state_path, agent_id.as_deref())
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    /// 应用情绪事件
    #[napi]
    pub fn apply_event(&mut self, name: String, intensity: Option<f64>) -> Result<EmotionSnapshot> {
        let i = intensity.unwrap_or(1.0) as f32;
        let snap = self
            .inner
            .apply_event(&name, i)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(to_js_snap(&snap))
    }

    /// 获取当前状态
    #[napi]
    pub fn get_state(&mut self) -> EmotionSnapshot {
        let snap = self.inner.get_state();
        to_js_snap(&snap)
    }

    /// 直接修改单个维度
    #[napi]
    pub fn modify(&mut self, dimension: String, delta: f64) -> Result<EmotionSnapshot> {
        let snap = self
            .inner
            .modify(&dimension, delta as f32)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(to_js_snap(&snap))
    }

    /// 重置至 baseline
    #[napi]
    pub fn reset(&mut self, dimensions: Option<Vec<String>>) -> EmotionSnapshot {
        let snap = match dimensions {
            Some(ref dims) => self.inner.reset(Some(dims)),
            None => self.inner.reset(None),
        };
        to_js_snap(&snap)
    }

    /// 设置人格特质
    #[napi]
    pub fn set_personality(&mut self, trait_name: String, value: f64) -> Result<()> {
        self.inner
            .set_personality(&trait_name, value as f32)
            .map_err(|e| Error::from_reason(e))
    }

    /// 获取人格配置
    #[napi]
    pub fn get_personality(&self) -> OceanProfile {
        let p = self.inner.get_personality();
        OceanProfile {
            openness: f64::from(p.openness),
            conscientiousness: f64::from(p.conscientiousness),
            extraversion: f64::from(p.extraversion),
            agreeableness: f64::from(p.agreeableness),
            neuroticism: f64::from(p.neuroticism),
        }
    }

    /// 扫描其他 Agent
    #[napi]
    pub fn get_other_agents(&self, scan_dir: String) -> Vec<AgentSummary> {
        multi_agent::scan_other_agents(&scan_dir, &self.inner.state_path)
            .into_iter()
            .map(|a| AgentSummary {
                agent_id: a.agent_id,
                v: f64::from(a.v),
                a: f64::from(a.a),
                d: f64::from(a.d),
                last_updated_ms: a.last_updated_ms,
            })
            .collect()
    }

    /// Prompt XML 格式化
    #[napi]
    pub fn format_prompt(&mut self) -> String {
        prompt::format_emotion_prompt(&mut self.inner)
    }

    /// 保存状态
    #[napi]
    pub fn save(&mut self) -> Result<()> {
        persistence::save_state(&mut self.inner).map_err(|e| Error::from_reason(e.to_string()))
    }

    /// 加载状态
    #[napi]
    pub fn load(&mut self) -> Result<()> {
        persistence::load_state(&mut self.inner).map_err(|e| Error::from_reason(e.to_string()))
    }
}
