use serde::{Deserialize, Serialize};

use crate::vad::VadState;

/// 默认反刍触发阈值
pub const RUMINATION_THRESHOLD: f32 = 0.4;
/// 默认最大影响轮次
pub const DEFAULT_MAX_ROUNDS: u32 = 3;
/// 默认每轮衰减因子
pub const DEFAULT_DECAY_FACTOR: f32 = 0.5;
/// 同时活跃的反刍条目上限
pub const MAX_ACTIVE_ENTRIES: usize = 8;

/// 单条反刍 (情绪余波) 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuminationEntry {
    /// 产生反刍的事件名
    pub source_event: String,
    /// 对 V/A/D 的余波增量 (初始强度)
    pub delta: VadState,
    /// 剩余影响轮次
    pub remaining_rounds: u32,
    /// 每轮衰减因子 (0.0 - 1.0)
    pub decay_factor: f32,
    /// 总轮次 (用于计算衰减幂次)
    pub total_rounds: u32,
}

impl RuminationEntry {
    /// 创建新的反刍条目
    #[must_use]
    pub fn new(source_event: String, delta: VadState) -> Self {
        Self {
            source_event,
            delta,
            remaining_rounds: DEFAULT_MAX_ROUNDS,
            decay_factor: DEFAULT_DECAY_FACTOR,
            total_rounds: DEFAULT_MAX_ROUNDS,
        }
    }

    /// 计算当前轮次的余波贡献
    #[must_use]
    pub fn current_contribution(&self) -> (f32, f32, f32) {
        let elapsed = self.total_rounds - self.remaining_rounds;
        let power = self.decay_factor.powi(elapsed as i32);
        (
            self.delta.v * power,
            self.delta.a * power,
            self.delta.d * power,
        )
    }
}

/// 检查事件增量是否触发反刍
#[must_use]
pub fn should_ruminate(delta: &VadState, threshold: f32) -> bool {
    delta.v.abs() > threshold || delta.a.abs() > threshold || delta.d.abs() > threshold
}

/// 推进所有活跃的反刍条目一轮，并将余波叠加到状态上
pub fn advance_ruminations(state: &mut VadState, entries: &mut Vec<RuminationEntry>) {
    entries.retain_mut(|entry| {
        if entry.remaining_rounds == 0 {
            return false;
        }
        let (dv, da, dd) = entry.current_contribution();
        state.apply_delta(dv, da, dd);
        entry.remaining_rounds -= 1;
        entry.remaining_rounds > 0
    });
}

/// 添加反刍条目（受上限约束，淘汰最旧的）
pub fn add_rumination(entries: &mut Vec<RuminationEntry>, entry: RuminationEntry) {
    if entries.len() >= MAX_ACTIVE_ENTRIES {
        entries.remove(0);
    }
    entries.push(entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ruminate() {
        let big = VadState::new(0.5, 0.0, 0.0);
        assert!(should_ruminate(&big, RUMINATION_THRESHOLD));

        let small = VadState::new(0.1, 0.1, 0.1);
        assert!(!should_ruminate(&small, RUMINATION_THRESHOLD));
    }

    #[test]
    fn test_advance_removes_expired() {
        let mut state = VadState::neutral();
        let mut entries = vec![RuminationEntry {
            source_event: "test".to_string(),
            delta: VadState::new(0.1, 0.0, 0.0),
            remaining_rounds: 1,
            decay_factor: 0.7,
            total_rounds: 5,
        }];

        advance_ruminations(&mut state, &mut entries);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_advance_applies_contribution() {
        let mut state = VadState::neutral();
        let mut entries = vec![RuminationEntry::new(
            "anger".to_string(),
            VadState::new(-0.5, 0.6, 0.3),
        )];

        advance_ruminations(&mut state, &mut entries);
        // 第一轮 (elapsed=0): power = 0.7^0 = 1.0, 所以 delta 完整应用
        assert!(state.v < 0.0);
        assert!(state.a > 0.0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].remaining_rounds, DEFAULT_MAX_ROUNDS - 1);
    }

    #[test]
    fn test_max_active_entries() {
        let mut entries = Vec::new();
        for i in 0..MAX_ACTIVE_ENTRIES + 3 {
            add_rumination(
                &mut entries,
                RuminationEntry::new(format!("event_{i}"), VadState::neutral()),
            );
        }
        assert_eq!(entries.len(), MAX_ACTIVE_ENTRIES);
    }
}
