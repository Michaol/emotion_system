//! 情感记忆模块
//!
//! 提供 EmotionalMemory 数据结构、显著性计算、幂律衰减、
//! 召回强化以及基于 cosine similarity 的记忆检索。

use serde::{Deserialize, Serialize};

use crate::vad::VadState;

/// 计算情绪显著性
#[must_use]
pub fn compute_salience(delta: &VadState, personality_weight: f32) -> f32 {
    let raw = (delta.v * delta.v + delta.a * delta.a + delta.d * delta.d).sqrt();
    raw * personality_weight
}

/// 情感记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalMemory {
    /// 事件名称
    pub event_name: String,
    /// 事件发生时的 VAD 状态
    pub vad_at_event: VadState,
    /// 事件带来的 VAD 变化
    pub delta: VadState,
    /// 时间戳 (ms)
    pub timestamp_ms: i64,
    /// 显著性
    pub salience: f32,
    /// 被检索次数 (用于召回强化)
    pub recall_count: u32,
    /// 当前留存强度 (0.0 ~ 1.0)
    pub retention: f32,
}

impl EmotionalMemory {
    #[must_use]
    pub fn new(
        event_name: String,
        vad_at_event: VadState,
        delta: VadState,
        timestamp_ms: i64,
    ) -> Self {
        let salience = compute_salience(&delta, 1.0);
        Self {
            event_name,
            vad_at_event,
            delta,
            timestamp_ms,
            salience,
            recall_count: 0,
            retention: 1.0,
        }
    }

    /// 更新留存强度 (幂律衰减)
    /// retention = (1 + alpha * hours)^(-beta_salience)
    pub fn update_retention(&mut self, hours_elapsed: f64, alpha: f32, beta_base: f32) {
        let beta = beta_base / (1.0 + self.salience * 2.0);
        let t = hours_elapsed as f32;
        self.retention = (1.0 + alpha * t).powf(-beta);
    }

    /// 召回强化：每次检索时调用
    pub fn recall(&mut self) {
        self.recall_count += 1;
    }

    /// 有效强度 = retention × (1 + recall_count × 0.2)
    #[must_use]
    pub fn effective_strength(&self) -> f32 {
        self.retention * (1.0 + self.recall_count as f32 * 0.2)
    }

    /// 是否应该被回收 (retention 过低)
    #[must_use]
    pub fn should_gc(&self, threshold: f32) -> bool {
        self.retention < threshold
    }
}

/// 检索与当前 VAD 最相关的记忆 (按 effective_strength 排序)
#[must_use]
pub fn retrieve_memories<'a>(
    memories: &'a [EmotionalMemory],
    current: &VadState,
    top_k: usize,
) -> Vec<&'a EmotionalMemory> {
    let mut scored: Vec<(&EmotionalMemory, f32)> = memories
        .iter()
        .map(|m| {
            let dot = current.v * m.vad_at_event.v
                + current.a * m.vad_at_event.a
                + current.d * m.vad_at_event.d;
            let norm_c = (current.v.powi(2) + current.a.powi(2) + current.d.powi(2))
                .sqrt()
                .max(1e-6);
            let norm_m =
                (m.vad_at_event.v.powi(2) + m.vad_at_event.a.powi(2) + m.vad_at_event.d.powi(2))
                    .sqrt()
                    .max(1e-6);
            let cosine = dot / (norm_c * norm_m);
            let score = cosine.max(0.0) * m.effective_strength();
            (m, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(top_k).map(|(m, _)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let mem = EmotionalMemory::new(
            "joy".to_string(),
            VadState::new(0.4, 0.2, 0.1),
            VadState::new(0.4, 0.2, 0.1),
            1000,
        );
        assert_eq!(mem.event_name, "joy");
        assert_eq!(mem.recall_count, 0);
        assert!((mem.retention - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_salience_calculation() {
        let delta = VadState::new(0.6, 0.3, 0.0);
        let salience = compute_salience(&delta, 1.0);
        let expected = (0.6f32 * 0.6 + 0.3 * 0.3).sqrt();
        assert!((salience - expected).abs() < 0.01);
    }

    #[test]
    fn test_retention_decay() {
        let mut mem = EmotionalMemory::new(
            "test".to_string(),
            VadState::neutral(),
            VadState::new(0.5, 0.0, 0.0),
            0,
        );
        mem.update_retention(10.0, 0.5, 1.0);
        assert!(mem.retention < 1.0);
        assert!(mem.retention > 0.0);
    }

    #[test]
    fn test_recall_reinforcement() {
        let mut mem = EmotionalMemory::new(
            "test".to_string(),
            VadState::neutral(),
            VadState::neutral(),
            0,
        );
        mem.recall();
        assert_eq!(mem.recall_count, 1);
        assert!(mem.effective_strength() > mem.retention);
    }

    #[test]
    fn test_should_gc() {
        let mut mem = EmotionalMemory::new(
            "test".to_string(),
            VadState::neutral(),
            VadState::neutral(),
            0,
        );
        mem.retention = 0.01;
        assert!(mem.should_gc(0.05));
        assert!(!mem.should_gc(0.005));
    }

    #[test]
    fn test_retrieve_memories() {
        let memories = vec![
            EmotionalMemory::new(
                "joy".to_string(),
                VadState::new(0.8, 0.5, 0.4),
                VadState::new(0.4, 0.2, 0.1),
                1000,
            ),
            EmotionalMemory::new(
                "sadness".to_string(),
                VadState::new(-0.6, -0.3, -0.3),
                VadState::new(-0.3, -0.1, -0.1),
                2000,
            ),
        ];
        let current = VadState::new(0.7, 0.4, 0.3);
        let result = retrieve_memories(&memories, &current, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].event_name, "joy"); // 更接近当前 VAD
    }
}
