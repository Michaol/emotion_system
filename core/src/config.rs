use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Unknown event: {0}")]
    UnknownEvent(String),
}

/// 单个事件的 VAD 增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDelta {
    #[serde(default)]
    pub delta_v: f32,
    #[serde(default)]
    pub delta_a: f32,
    #[serde(default)]
    pub delta_d: f32,
}

/// 事件映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub events: HashMap<String, EventDelta>,
}

impl EventConfig {
    /// 从 JSON 文件加载
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 查找事件增量
    pub fn get_delta(&self, name: &str) -> Result<&EventDelta, ConfigError> {
        self.events
            .get(name)
            .ok_or_else(|| ConfigError::UnknownEvent(name.to_string()))
    }
}

/// 行为条件 (可选边界)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BehaviorCondition {
    #[serde(default)]
    pub v_min: Option<f32>,
    #[serde(default)]
    pub v_max: Option<f32>,
    #[serde(default)]
    pub a_min: Option<f32>,
    #[serde(default)]
    pub a_max: Option<f32>,
    #[serde(default)]
    pub d_min: Option<f32>,
    #[serde(default)]
    pub d_max: Option<f32>,
}

impl BehaviorCondition {
    /// 检查 VAD 状态是否满足此条件
    #[must_use]
    pub fn matches(&self, v: f32, a: f32, d: f32) -> bool {
        self.v_min.is_none_or(|min| v >= min)
            && self.v_max.is_none_or(|max| v <= max)
            && self.a_min.is_none_or(|min| a >= min)
            && self.a_max.is_none_or(|max| a <= max)
            && self.d_min.is_none_or(|min| d >= min)
            && self.d_max.is_none_or(|max| d <= max)
    }
}

/// 行为输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorOutput {
    pub tone: String,
    pub speed: String,
    pub description: String,
}

/// 行为规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorRule {
    pub condition: BehaviorCondition,
    pub tone: String,
    pub speed: String,
    pub description: String,
}

/// 行为映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub version: String,
    pub behaviors: Vec<BehaviorRule>,
    pub default: BehaviorOutput,
}

impl BehaviorConfig {
    /// 从 JSON 文件加载
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 匹配 VAD 到行为 (第一个命中的规则)
    ///
    /// 注意：此函数返回匹配规则的 BehaviorOutput。由于 Rust 借用检查器限制，
    /// 无法直接返回 &BehaviorOutput（规则是 Vec 中的独立字段）。
    /// 如需零拷贝，请使用 resolve() 返回 &str 元组。
    #[must_use]
    pub fn match_behavior(&self, v: f32, a: f32, d: f32) -> BehaviorOutput {
        for rule in &self.behaviors {
            if rule.condition.matches(v, a, d) {
                return BehaviorOutput {
                    tone: rule.tone.clone(),
                    speed: rule.speed.clone(),
                    description: rule.description.clone(),
                };
            }
        }
        self.default.clone()
    }

    /// 匹配 VAD 到行为描述 (返回 tone, speed, description)
    #[must_use]
    pub fn resolve(&self, v: f32, a: f32, d: f32) -> (&str, &str, &str) {
        for rule in &self.behaviors {
            if rule.condition.matches(v, a, d) {
                return (&rule.tone, &rule.speed, &rule.description);
            }
        }
        (
            &self.default.tone,
            &self.default.speed,
            &self.default.description,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavior_condition_matches() {
        let c = BehaviorCondition {
            v_min: Some(0.3),
            a_min: Some(0.5),
            ..Default::default()
        };
        assert!(c.matches(0.5, 0.6, 0.0));
        assert!(!c.matches(0.1, 0.6, 0.0));
        assert!(!c.matches(0.5, 0.2, 0.0));
    }

    #[test]
    fn test_event_delta_parse() {
        let json = r#"{
            "version": "1.0",
            "events": {
                "joy": { "delta_v": 0.4, "delta_a": 0.2, "delta_d": 0.1 }
            }
        }"#;
        let config: EventConfig = serde_json::from_str(json).unwrap();
        let d = config.get_delta("joy").unwrap();
        assert!((d.delta_v - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_unknown_event() {
        let config = EventConfig {
            version: "1.0".to_string(),
            description: None,
            events: HashMap::new(),
        };
        assert!(config.get_delta("nonexistent").is_err());
    }

    #[test]
    fn test_match_behavior_returns_matched_rule() {
        let config = BehaviorConfig {
            version: "1.0".to_string(),
            behaviors: vec![BehaviorRule {
                condition: BehaviorCondition {
                    v_min: Some(0.3),
                    a_min: Some(0.5),
                    ..Default::default()
                },
                tone: "excited".to_string(),
                speed: "fast".to_string(),
                description: "High energy".to_string(),
            }],
            default: BehaviorOutput {
                tone: "neutral".to_string(),
                speed: "moderate".to_string(),
                description: "Stable".to_string(),
            },
        };

        // VAD matches the rule → should return rule's output
        let result = config.match_behavior(0.5, 0.6, 0.0);
        assert_eq!(result.tone, "excited");
        assert_eq!(result.speed, "fast");

        // VAD doesn't match → should return default
        let result2 = config.match_behavior(0.0, 0.1, 0.0);
        assert_eq!(result2.tone, "neutral");
    }
}
